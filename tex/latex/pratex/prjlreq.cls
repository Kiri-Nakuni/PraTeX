%% Japanese document class based on requirements for Japanese text layout
%%
%% Upstream jlreq and jlreq-trimmarks:
%% Copyright 2017-2026, Noriyuki Abe.
%% All rights reserved.
%% Distributed under the BSD 2-Clause License.
%%
%% This file is a modified PraTeX adaptation of jlreq.cls from
%% ac06fd0770096be8a118197d670a34da06384e02.  It is not an independent
%% implementation and is not the upstream jlreq class.  See
%% prjlreq-UPSTREAM-LICENSE.txt and docs/prjlreq-provenance.md.
%%
%% Initial adaptation: PraTeX-only identity, horizontal article loading, and
%% the upstream horizontal (x)kanjiskip and paragraph-indent defaults.  The
%% unsupported upstream surface is rejected instead of selecting another
%% engine branch.

\NeedsTeXFormat{LaTeX2e}[2023-11-01]
\ProvidesClass{prjlreq}
  [2026/08/24 v0.1 incomplete PraTeX adaptation of jlreq 2026-07-17]

% PraTeX owns this identity primitive.  Its value is a release gate, so zero
% is a valid development engine and must not be rejected.
\ifdefined\pratexversion\else
  \ClassError{prjlreq}
    {PraTeX identity primitive \string\pratexversion\space is missing}
    {Run this adapted class with PraTeX.  Do not define another engine's
     version primitive or a local stand-in for \string\pratexversion.}
  \expandafter\endinput
\fi

\newif\ifprjlreq@narrow@kanjiskip
\prjlreq@narrow@kanjiskipfalse
\newif\ifprjlreq@unsupported
\prjlreq@unsupportedfalse

\def\prjlreq@unsupportedoption#1#2{%
  \prjlreq@unsupportedtrue
  \ClassError{prjlreq}
    {Class option `#1' is not supported by this PraTeX slice}
    {#2}}

% These names remain recognizable only so that an attempted engine selection
% fails at the class boundary.  They never select an upTeX, pTeX, or LuaTeX
% implementation branch and no foreign identity primitive is synthesized.
\DeclareOption{platex}{%
  \prjlreq@unsupportedoption{platex}{Use PraTeX without an engine option.}}
\DeclareOption{uplatex}{%
  \prjlreq@unsupportedoption{uplatex}{Use PraTeX without an engine option.}}
\DeclareOption{lualatex}{%
  \prjlreq@unsupportedoption{lualatex}{Use PraTeX without an engine option.}}

\DeclareOption{tate}{%
  \prjlreq@unsupportedoption{tate}{PraTeX vertical direction is not connected yet.}}
\DeclareOption{report}{%
  \prjlreq@unsupportedoption{report}{Only the horizontal article slice is connected.}}
\DeclareOption{book}{%
  \prjlreq@unsupportedoption{book}{Only the horizontal article slice is connected.}}
\DeclareOption{article}{}
\DeclareOption{narrow_kanjiskip}{\prjlreq@narrow@kanjiskiptrue}

\DeclareOption*{\PassOptionsToClass{\CurrentOption}{article}}
\ExecuteOptions{a4paper,10pt}
\ProcessOptions\relax
\ifprjlreq@unsupported
  \expandafter\endinput
\fi
\LoadClass{article}

\ifdefined\pratexjfont\else
  \ClassError{prjlreq}
    {PraTeX Japanese font primitive \string\pratexjfont\space is missing}
    {Use a PraTeX build with horizontal JFM support.}
  \expandafter\endinput
\fi
\ifdefined\kanjiskip\else
  \ClassError{prjlreq}
    {PraTeX native \string\kanjiskip\space is missing}
    {Use a PraTeX build with Japanese spacing parameters.}
  \expandafter\endinput
\fi
\ifdefined\xkanjiskip\else
  \ClassError{prjlreq}
    {PraTeX native \string\xkanjiskip\space is missing}
    {Use a PraTeX build with Japanese spacing parameters.}
  \expandafter\endinput
\fi

\RequirePackage{pratex-japanese}

% Adapted from jlreq.cls at ac06fd0, character settings around upstream
% lines 749--768.  PraTeX's central dimension scanner owns zw/zh.
\providecommand*{\zw}{zw}
\providecommand*{\zh}{zh}
\let\jlreq@zw=\zw
\let\jlreq@zh=\zh

\ifprjlreq@narrow@kanjiskip
  \providecommand*{\jlreqkanjiskip}{0pt plus .1\jlreq@zw minus .01\jlreq@zw}
\else
  \providecommand*{\jlreqkanjiskip}{0pt plus .25\jlreq@zw minus 0pt}
\fi
\providecommand*{\jlreqxkanjiskip}{%
  .25\jlreq@zw plus .25\jlreq@zw minus .125\jlreq@zw}
\AtBeginDocument{%
  \kanjiskip=\jlreqkanjiskip\relax
  \xkanjiskip=\jlreqxkanjiskip\relax
}

% Adapted from the horizontal paragraph defaults around upstream lines
% 1227--1229.  Heading, page-style, and kihon-hanmen APIs are not claimed by
% this first slice.
\setlength{\parindent}{1\jlreq@zw}
\emergencystretch=3\jlreq@zh

\endinput

