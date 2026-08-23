%% prjsarticle.cls -- PraTeX native horizontal Japanese article class
%%
%% Copyright (C) 2026 PraTeX contributors
%% SPDX-License-Identifier: GPL-3.0-only
%%
%% This is an original PraTeX class.  Its behavioural research considered
%% jsclasses 2025-05-10 (BSD-2-Clause), but no jsclasses source was copied.

\NeedsTeXFormat{LaTeX2e}[2023-11-01]
\ProvidesClass{prjsarticle}
  [2026/08/24 v0.2 PraTeX native horizontal Japanese article]

% Do not impersonate another engine to enter its package branches.
% PraTeX owns this identity primitive.  Presence identifies the engine;
% the value is a release gate and remains 0 until PraTeX 1 is complete.
\ifdefined\pratexversion\else
  \ClassError{prjsarticle}
    {PraTeX identity primitive \string\pratexversion\space is missing}
    {Run this class with PraTeX.  Do not define another engine's version
     primitive or a local stand-in for \string\pratexversion.}
  \expandafter\endinput
\fi
% The class uses the LaTeX author interface supplied by article, while all
% Japanese engine decisions remain PraTeX-native hooks below.
\PassOptionsToClass{a4paper}{article}
\DeclareOption*{\PassOptionsToClass{\CurrentOption}{article}}
\ProcessOptions\relax
\LoadClass{article}

\ifdefined\kanjiskip\else
  \ClassError{prjsarticle}
    {PraTeX native \string\kanjiskip\space is missing}
    {Merge the PraTeX Japanese spacing parameter slice before using this class.}
  \expandafter\endinput
\fi
\ifdefined\xkanjiskip\else
  \ClassError{prjsarticle}
    {PraTeX native \string\xkanjiskip\space is missing}
    {Merge the PraTeX Japanese spacing parameter slice before using this class.}
  \expandafter\endinput
\fi

% Format or engine adapters replace these hooks without making the class
% pretend to be pTeX, upTeX, pdfTeX, XeTeX, or LuaTeX.
\newcommand\pratexlatinfonthook{\rmfamily}
\newcommand\pratexjapanesefonthook{}
\newcommand\pratexsetlatinfonthook[1]{%
  \gdef\pratexlatinfonthook{#1}}
\newcommand\pratexsetjapanesefonthook[1]{%
  \gdef\pratexjapanesefonthook{#1}}
% TeX Live's upjisr-h is the explicit compatibility default.  Loading this
% after the hook definition lets the package register without engine
% impersonation.  A document adapter may still replace the hook afterwards.
\RequirePackage{pratex-japanese}
\newcommand\pratexbodyfont{%
  \pratexlatinfonthook\pratexjapanesefonthook}
\newcommand\pratexheadingfont{%
  \sffamily\bfseries\pratexjapanesefonthook}
\newcommand\pratextitlefont{%
  \LARGE\bfseries\pratexlatinfonthook\pratexjapanesefonthook}
\newcommand\pratexauthorfont{%
  \large\pratexlatinfonthook\pratexjapanesefonthook}
\newcommand\pratexdatefont{%
  \normalsize\pratexlatinfonthook\pratexjapanesefonthook}

% Horizontal Japanese baseline.  JFM-aware zw is resolved by the engine at
% document start, after a format has installed its Japanese font hook.
\renewcommand\baselinestretch{1.25}
\normalsize
\AtBeginDocument{%
  \pratexbodyfont
  \kanjiskip=0zw plus .1zw minus .05zw\relax
  \xkanjiskip=.25zw plus .15zw minus .1zw\relax
}

% A4, roughly forty full-width characters per line, with a stable text block.
\setlength\textwidth{40em}
\setlength\oddsidemargin{\dimexpr(\paperwidth-\textwidth)/2-1in\relax}
\setlength\evensidemargin{\oddsidemargin}
\setlength\headheight{12pt}
\setlength\headsep{8mm}
\setlength\topmargin{\dimexpr25mm-1in-\headheight-\headsep\relax}
\setlength\textheight{43\baselineskip}
\addtolength\textheight{\topskip}
\setlength\footskip{2\baselineskip}
\setlength\marginparwidth{20mm}
\setlength\marginparsep{4mm}
\setlength\parindent{1em}
\setlength\parskip{0pt}
\setlength\columnsep{2em}
\flushbottom

% Title rows have fixed outer heights.  This keeps the body start and the DVI
% regression oracle stable even when an adapter changes Japanese font metrics.
\newlength\pratextitletopskip
\newlength\pratextitlerowheight
\newlength\pratexauthorrowheight
\newlength\pratexdaterowheight
\newlength\pratextitleafterskip
\setlength\pratextitletopskip{1.5\baselineskip}
\setlength\pratextitlerowheight{3.5\baselineskip}
\setlength\pratexauthorrowheight{2.5\baselineskip}
\setlength\pratexdaterowheight{1.5\baselineskip}
\setlength\pratextitleafterskip{1.5\baselineskip}

\newcommand\pratex@titlerow[2]{%
  \vbox to #1{%
    \vfil
    \hbox to \textwidth{%
      \hfil
      \vbox{%
        \hsize=.9\textwidth
        \parindent=0pt
        \centering #2\par
      }%
      \hfil
    }%
    \vfil
  }%
}

\renewcommand\maketitle{%
  \clearpage
  \thispagestyle{plain}%
  \begingroup
    \parindent=0pt
    \vspace*{\pratextitletopskip}%
    \vbox{%
      \offinterlineskip
      \pratex@titlerow{\pratextitlerowheight}%
        {{\pratextitlefont\@title}}%
      \pratex@titlerow{\pratexauthorrowheight}%
        {{\pratexauthorfont\@author}}%
      \pratex@titlerow{\pratexdaterowheight}%
        {{\pratexdatefont\@date}}%
    }%
  \endgroup
  \par
  \vspace{\pratextitleafterskip}%
  \setcounter{footnote}{0}%
  \global\let\maketitle\relax
  \global\let\@title\@empty
  \global\let\@author\@empty
  \global\let\@date\@empty
}

% Headings use the same Japanese-font boundary as ordinary text.  The spacing
% is expressed in the document baseline rather than copied from another class.
\renewcommand\section{%
  \@startsection{section}{1}{\z@}%
    {1.5\baselineskip \@plus .4\baselineskip \@minus .2\baselineskip}%
    {.5\baselineskip}%
    {\normalfont\Large\pratexheadingfont}}
\renewcommand\subsection{%
  \@startsection{subsection}{2}{\z@}%
    {1.25\baselineskip \@plus .3\baselineskip \@minus .15\baselineskip}%
    {.35\baselineskip}%
    {\normalfont\large\pratexheadingfont}}
\renewcommand\subsubsection{%
  \@startsection{subsubsection}{3}{\z@}%
    {1.0\baselineskip \@plus .25\baselineskip \@minus .1\baselineskip}%
    {.25\baselineskip}%
    {\normalfont\normalsize\pratexheadingfont}}
\renewcommand\paragraph{%
  \@startsection{paragraph}{4}{\z@}%
    {.75\baselineskip \@plus .2\baselineskip \@minus .1\baselineskip}%
    {-.75em}%
    {\normalfont\normalsize\pratexheadingfont}}

% Compact, two-em first-level lists suit a forty-character Japanese measure.
\setlength\leftmargini{2em}
\setlength\leftmarginii{1.75em}
\setlength\labelsep{.5em}
\setlength\itemsep{.2\baselineskip}
\setlength\parsep{0pt}
\setlength\topsep{.5\baselineskip}
\renewcommand\descriptionlabel[1]{%
  \hspace\labelsep\normalfont\pratexheadingfont #1}

\renewcommand\contentsname{目次}
\renewcommand\refname{参考文献}
\renewcommand\figurename{図}
\renewcommand\tablename{表}
\renewcommand\abstractname{概要}

\endinput
