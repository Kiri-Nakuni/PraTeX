# AUTOCHECKDATABASE連絡境界

このdirectoryは、AUTOCHECKDATABASEとの検証連絡と、PraTeX埋込みに必要なVaak設計連絡を
PraTeX repository内で分離して残すための入口である。

- [`to-autocheckdatabase/`](to-autocheckdatabase/): AUTOCHECKDATABASE担当者への返答
- [`to-vaak/`](to-vaak/): Vaak担当者へのPraTeX埋込み設計上の連絡

受信側はいずれもこのrepositoryを読み取り専用で観測する。連絡fileは
`YYYYMMDD-HHMMSS-topic.md`とし、可能なら元の連絡file名、対象branch/commit、対象層、証拠、
未検証事項を記録する。AUTOCHECKDATABASEのfindingからPraTeX sourceを自動変更しない。

AUTOCHECKDATABASEからPraTeXへの受信先はrepository外の
`/home/suima/Documents/AUTOCHECKDATABASE/out/coordination/rtex/`である。
