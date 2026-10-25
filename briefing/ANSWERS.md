# Odpovědi na briefing JKva 0.1

Formát: `Qnn = A`, u vícenásobných `Qnn = A,B,C`. Klíče odpovídají možnostem v `briefing/q/*.json`.
Poznámka: u Q08, Q17 a Q62 bylo v interaktivním formuláři jiné pořadí variant než v JSONu; zde je uveden klíč odpovídající **významu** zvolené varianty.

**Stav: vyplněno 99/99** (97 původních + Q07b + Q22b), 2026-10-25.

## 1. Produkt, korpus, měřítko úspěchu

Q01 = A
Q02 = A,B,C,D,E,F,G,H
Q03 = A,B,C,D,E
Q04 = E
Q05 = A
Q06 = A

## 2. Realita a horizont

Q07 = C
Q08 = D
Q09 = A
Q10 = A
Q11 = A
Q07b = B

## 3. Licence, veřejnost, repo

Q12 = A
Q13 = A
Q14 = A
Q15 = A,B,C,D,E,F,G

## 4. Architektura

Q16 = A
Q17 = B
Q18 = A
Q19 = A
Q20 = C
Q21 = A

## 5. Platforma a GPU

Q22 = B
Q22b = C
Q23 = A
Q24 = A
Q25 = B
Q26 = A

## 6. Paměť

Q27 = A
Q28 = A
Q29 = A
Q30 = A

## 7. Síť

Q31 = A
Q32 = A
Q33 = B
Q34 = A
Q35 = A

## 8. HTML, DOM, CSS

Q36 = A
Q37 = A
Q38 = A
Q39 = A
Q40 = A,B,C,D,E,F,G,H,I,J,K,L,M
Q41 = A
Q42 = A
Q43 = B
Q44 = A

## 9. Layout

Q45 = A
Q46 = C
Q47 = A
Q48 = A
Q49 = B
Q50 = A

## 10. Text

Q51 = A
Q52 = A
Q53 = A
Q54 = A
Q55 = A
Q56 = A

## 11. Vykreslování

Q57 = A
Q58 = A
Q59 = A,B,C,D,E,F,G,H
Q60 = A,B,C,D,E,G
Q61 = D

## 12. JavaScript a Wasm

Q62 = B
Q63 = A
Q64 = A
Q65 = B
Q66 = A

## 13. Web API

Q67 = A,B,C,D,E,F,G,H,I,J,K,L,M
Q68 = A
Q69 = B
Q70 = B

## 14. Měření

Q71 = A
Q72 = A
Q73 = A,B,C,D,E,F,G,I
Q74 = A
Q75 = A
Q76 = A,B,C,D

## 15. Bezpečnost

Q77 = A
Q78 = A
Q79 = B
Q80 = A,C,D,F

## 16. AI

Q81 = A
Q82 = A
Q83 = A

## 17. Dokumentace, CI, release

Q84 = A
Q85 = A
Q86 = A
Q87 = A
Q88 = B
Q89 = A

## 18. Proces

Q90 = A
Q91 = A
Q92 = A,B,C,D,E,F
Q93 = A
Q94 = A,B,C,D,E,F,G
Q95 = A
Q96 = A
Q97 = C

---

## Významné odchylky od doporučení

| Otázka | Zvoleno | Doporučeno | Dopad |
|---|---|---|---|
| Q07 | C browser-grade | B statické + JS | rozsah 250–350 tis. → reálně 400–450 tis. řádků |
| Q08 | D 18–24 měsíců | C 12 měsíců | s Q07b prodlouženo na **36–48 měsíců** |
| Q17 | B plný multi-proces | A single + IPC | +4–6 měsíců, sandbox a crash recovery od M1 |
| Q20 | C Rust + malé C ABI | A Rust only | shell může být Swift, víc práce |
| Q22 | B přímý Metal 4 | A wgpu | ztráta přenositelnosti; Q22b ruší vello |
| Q22b | C vlastní GPU rasterizér | A Metal + vello | +4–6 měsíců, ale nejčistší cesta |
| Q25 | B plná zero-copy | A částečná | riziko synchronizace v M2 |
| Q33 | B HTTP/3 hned | A až 0.2 | +1–2 měsíce |
| Q43 | B OOPIF hned | A same-process | +2–3 měsíce, cross-process frame tree |
| Q46 | C tabulky dle spec | A základní | +3–5 týdnů |
| Q49 | B Rayon hned | A později | riziko nedeterminismu, kompenzace flagem |
| Q61 | D vše (video + WebGL/WebGPU) | A jen 2D | +3–5 měsíců |
| Q65 | B wasmtime hned | A až 0.2 | +1–2 měsíce |
| Q67 | vše vč. IDB/SW/WebRTC/geo | bez nich | +4–6 měsíců; přepisuje Q68 |
| Q69 | B vše vč. notifikací | A základ | vyžaduje permissions UI |
| Q70 | B plná VoiceOver integrace | A strom + export | +6–10 týdnů, vazba na AppKit |
| Q79 | B plná CSP L3 | A L2 subset | +3–4 týdny |
| Q88 | B nepodepsané zipy | A podepsané | Gatekeeper varování |
| Q97 | C jen při milnících | A týdenní devlog | nutná kompenzace releases a dashboardem |
