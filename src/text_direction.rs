//! TeX--XeT direction nodeをbackend非依存のshipout順へ変換する。

use crate::nodes::{MathNodeKind, Node, TextDirection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectionIssue {
    UnexpectedEnd(TextDirection),
    MismatchedEnd {
        open: TextDirection,
        close: TextDirection,
    },
    UnclosedBegin(TextDirection),
    UnexpectedMathEnd,
    UnclosedMath,
    UnsupportedDiscretionary,
}

pub(crate) struct DirectedOrder<'a> {
    pub(crate) nodes: Vec<&'a Node>,
    pub(crate) issues: Vec<DirectionIssue>,
}

enum Item<'a> {
    Node(&'a Node),
    /// 内側の方向意味を保つatomicな区間。
    ///
    /// closeごとに子treeをcopyせず、最後の一回だけ平坦化する。
    Group(Frame<'a>),
}

struct Frame<'a> {
    direction: TextDirection,
    items: Vec<Item<'a>>,
}

impl<'a> Frame<'a> {
    fn new(direction: TextDirection) -> Self {
        Self {
            direction,
            items: Vec::new(),
        }
    }

    fn flatten(self, node_capacity: usize) -> Vec<&'a Node> {
        enum Pending<'a> {
            Frame(Frame<'a>),
            Item(Item<'a>),
        }

        let mut flattened = Vec::with_capacity(node_capacity);
        let mut pending = vec![Pending::Frame(self)];
        while let Some(next) = pending.pop() {
            match next {
                Pending::Item(Item::Node(node)) => flattened.push(node),
                Pending::Item(Item::Group(frame)) | Pending::Frame(frame) => {
                    match frame.direction {
                        TextDirection::LeftToRight => {
                            for item in frame.items.into_iter().rev() {
                                pending.push(Pending::Item(item));
                            }
                        }
                        TextDirection::RightToLeft => {
                            for item in frame.items {
                                pending.push(Pending::Item(item));
                            }
                        }
                    }
                }
            }
        }
        flattened
    }
}

fn marker(node: &Node) -> Option<(bool, TextDirection)> {
    let Node::Math(math) = node else {
        return None;
    };
    match math.kind {
        MathNodeKind::Begin(direction) => Some((true, direction)),
        MathNodeKind::End(direction) => Some((false, direction)),
        MathNodeKind::Before | MathNodeKind::After => None,
    }
}

pub(crate) fn is_direction_boundary(node: &Node) -> bool {
    marker(node).is_some()
}

pub(crate) fn contains_direction_boundaries(hlist: &[Node]) -> bool {
    hlist.iter().any(is_direction_boundary)
}

fn close_top<'a>(stack: &mut Vec<Frame<'a>>) {
    let frame = stack.pop().expect("direction stack has a nested frame");
    stack
        .last_mut()
        .expect("direction stack always has a root frame")
        .items
        .push(Item::Group(frame));
}

fn discard_frame_tree<'a>(mut frames: Vec<Frame<'a>>) {
    while let Some(frame) = frames.pop() {
        for item in frame.items {
            if let Item::Group(child) = item {
                frames.push(child);
            }
        }
    }
}

/// 方向nodeが無い通常listでは`None`を返し、allocationも並べ替えも行わない。
///
/// TeX--XeTは反転DVI opcodeを使わずengine内で明示的に順序を決める。同じ順序をDVI/PDF
/// backendが共有するため、backend側へ方向判断を複製しない。
pub(crate) fn directed_order(hlist: &[Node]) -> Option<DirectedOrder<'_>> {
    if !contains_direction_boundaries(hlist) {
        return None;
    }

    let mut stack = vec![Frame::new(TextDirection::LeftToRight)];
    let mut issues = Vec::new();
    let mut inline_math: Option<Frame<'_>> = None;
    for node in hlist {
        if let Some(math) = &mut inline_math {
            math.items.push(Item::Node(node));
            if matches!(
                node,
                Node::Math(crate::nodes::MathNode {
                    kind: MathNodeKind::After,
                    ..
                })
            ) {
                let math = inline_math.take().unwrap();
                stack
                    .last_mut()
                    .expect("direction stack always has a root frame")
                    .items
                    .push(Item::Group(math));
            }
            continue;
        }
        match node {
            Node::Math(crate::nodes::MathNode {
                kind: MathNodeKind::Before,
                ..
            }) => {
                // e-TeXのinline mathは外側のRTL区間内でも常にLTR。
                // math-surround幅を持つBefore/After nodeもatomic group内に残す。
                let mut math = Frame::new(TextDirection::LeftToRight);
                math.items.push(Item::Node(node));
                inline_math = Some(math);
                continue;
            }
            Node::Math(crate::nodes::MathNode {
                kind: MathNodeKind::After,
                ..
            }) => {
                issues.push(DirectionIssue::UnexpectedMathEnd);
                stack
                    .last_mut()
                    .expect("direction stack always has a root frame")
                    .items
                    .push(Item::Node(node));
                continue;
            }
            Node::Disc(_)
                if stack
                    .last()
                    .is_some_and(|frame| frame.direction == TextDirection::RightToLeft) =>
            {
                // restricted hboxで選ばれるno-break枝をRTLでどう平坦化するかは
                // 次sliceの契約。部分反転せず、このhlistの方向変換全体を破棄する。
                let nodes = hlist
                    .iter()
                    .filter(|candidate| !is_direction_boundary(candidate))
                    .collect();
                issues.push(DirectionIssue::UnsupportedDiscretionary);
                discard_frame_tree(stack);
                return Some(DirectedOrder { nodes, issues });
            }
            _ => {}
        }
        match marker(node) {
            Some((true, direction)) => stack.push(Frame::new(direction)),
            Some((false, close)) => {
                let Some(open) = stack.last().map(|frame| frame.direction) else {
                    unreachable!();
                };
                if stack.len() == 1 {
                    issues.push(DirectionIssue::UnexpectedEnd(close));
                } else if open != close {
                    issues.push(DirectionIssue::MismatchedEnd { open, close });
                } else {
                    close_top(&mut stack);
                }
            }
            None => stack
                .last_mut()
                .expect("direction stack always has a root frame")
                .items
                .push(Item::Node(node)),
        }
    }

    if let Some(math) = inline_math {
        issues.push(DirectionIssue::UnclosedMath);
        stack
            .last_mut()
            .expect("direction stack always has a root frame")
            .items
            .push(Item::Group(math));
    }

    while stack.len() > 1 {
        let direction = stack.last().unwrap().direction;
        issues.push(DirectionIssue::UnclosedBegin(direction));
        close_top(&mut stack);
    }
    let root = stack.pop().unwrap();
    Some(DirectedOrder {
        nodes: root.flatten(hlist.len()),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::{DiscNode, KernNode, MathNode};

    fn kern(width: i32) -> Node {
        Node::Kern(KernNode::new(width))
    }

    fn boundary(kind: MathNodeKind) -> Node {
        Node::Math(MathNode { kind, width: 0 })
    }

    fn widths(order: &DirectedOrder<'_>) -> Vec<i32> {
        order
            .nodes
            .iter()
            .map(|node| match node {
                Node::Kern(kern) => kern.width,
                _ => panic!("test order contains a non-kern node"),
            })
            .collect()
    }

    fn sequence(order: &DirectedOrder<'_>) -> Vec<i32> {
        order
            .nodes
            .iter()
            .map(|node| match node {
                Node::Kern(kern) => kern.width,
                Node::Math(MathNode {
                    kind: MathNodeKind::Before,
                    ..
                }) => -1,
                Node::Math(MathNode {
                    kind: MathNodeKind::After,
                    ..
                }) => -2,
                Node::Disc(_) => -3,
                _ => panic!("test order contains an unexpected node"),
            })
            .collect()
    }

    #[test]
    fn 方向nodeが無い通常listはallocation経路へ入らない() {
        assert!(directed_order(&[kern(1), kern(2)]).is_none());
    }

    #[test]
    fn 右から左の区間はengine内でnode順を反転する() {
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            kern(2),
            kern(3),
            boundary(MathNodeKind::End(TextDirection::RightToLeft)),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![3, 2, 1]);
        assert!(order.issues.is_empty());
    }

    #[test]
    fn 右向き区間内の左向き区間はatomicな内部順を保つ() {
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            boundary(MathNodeKind::Begin(TextDirection::LeftToRight)),
            kern(2),
            kern(3),
            boundary(MathNodeKind::End(TextDirection::LeftToRight)),
            kern(4),
            boundary(MathNodeKind::End(TextDirection::RightToLeft)),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![4, 2, 3, 1]);
        assert!(order.issues.is_empty());
    }

    #[test]
    fn 右向き区間でもinline_mathはatomicな左向き順を保つ() {
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            boundary(MathNodeKind::Before),
            kern(2),
            kern(3),
            boundary(MathNodeKind::After),
            kern(4),
            boundary(MathNodeKind::End(TextDirection::RightToLeft)),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(sequence(&order), vec![4, -1, 2, 3, -2, 1]);
        assert!(order.issues.is_empty());
    }

    #[test]
    fn rtl区間のdiscretionaryがあれば部分反転を破棄する() {
        let mut disc = DiscNode::new();
        disc.no_break = vec![kern(2), kern(3)];
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            Node::Disc(Box::new(disc)),
            kern(4),
            boundary(MathNodeKind::End(TextDirection::RightToLeft)),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(sequence(&order), vec![1, -3, 4]);
        assert_eq!(order.issues, vec![DirectionIssue::UnsupportedDiscretionary]);
    }

    #[test]
    fn 深いlr入れ子を再帰や子treeのcopyなしで平坦化する() {
        const DEPTH: usize = 4096;
        let mut list = Vec::with_capacity(2 * DEPTH + 1);
        for _ in 0..DEPTH {
            list.push(boundary(MathNodeKind::Begin(TextDirection::RightToLeft)));
        }
        list.push(kern(7));
        for _ in 0..DEPTH {
            list.push(boundary(MathNodeKind::End(TextDirection::RightToLeft)));
        }
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![7]);
        assert!(order.issues.is_empty());
    }

    #[test]
    fn 不一致endを別方向のbeginへ黙って対応づけない() {
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            boundary(MathNodeKind::End(TextDirection::LeftToRight)),
            kern(2),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![2, 1]);
        assert_eq!(
            order.issues,
            vec![
                DirectionIssue::MismatchedEnd {
                    open: TextDirection::RightToLeft,
                    close: TextDirection::LeftToRight,
                },
                DirectionIssue::UnclosedBegin(TextDirection::RightToLeft),
            ]
        );
    }

    #[test]
    fn 開いていないendはnode順を変えず診断する() {
        let list = vec![
            kern(1),
            boundary(MathNodeKind::End(TextDirection::RightToLeft)),
            kern(2),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![1, 2]);
        assert_eq!(
            order.issues,
            vec![DirectionIssue::UnexpectedEnd(TextDirection::RightToLeft)]
        );
    }

    #[test]
    fn 閉じないbeginはhlist終端で決定的に閉じる() {
        let list = vec![
            boundary(MathNodeKind::Begin(TextDirection::RightToLeft)),
            kern(1),
            kern(2),
        ];
        let order = directed_order(&list).unwrap();
        assert_eq!(widths(&order), vec![2, 1]);
        assert_eq!(
            order.issues,
            vec![DirectionIssue::UnclosedBegin(TextDirection::RightToLeft)]
        );
    }
}
