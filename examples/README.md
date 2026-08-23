# PraTeX examples

## plain format と Vaak

[`plain-vaak.tex`](plain-vaak.tex) は、PraTeX の plain format だけを読み、
`\directvaak`、`\vaakdef`、`let` / `var`、TeX host レジスタへの明示的な
alias を一つの文書で使う。

```console
pratex "&plain" examples/plain-vaak.tex
```

成功時の記録には次の印が出て、1ページのDVIが作られる。

```text
[plain-vaak answer=43;alias=22;host=22]
```

`count` と `dimen` は初めから host 値として見えるため、通常は直接
`count[5]` のように使える。例中の
`var registers : i32 array alias &= count;` は、同じhost領域へ別名を張る
明示形も動くことを示している。
