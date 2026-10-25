# Odpovědi na briefing v0.1

Formát: `Q01 = A`, multi: `Q40 = A,B,C`

## 1. Produkt, korpus a měřítko úspěchu

# Co je výstupem verze 0.1?  (doporučeno: A)
Q01 = 

# Jaký korpus webů musí v0.1 vykreslit správně?  (doporučeno: A,C,D,H)
Q02 = 

# Která veřejná tvrzení o výkonu budeme smět říkat?  (doporučeno: A,B,C,D,E)
Q03 = 

# Jaká je vlajková věta projektu (jedna, do README)?  (doporučeno: E)
Q04 = 

# Potvrzuješ, že těchto 12 věcí je vědomě mimo 0.1?  (doporučeno: A)
Q05 = 

# Potvrzuješ název JKva a rozdělení projektů?  (doporučeno: A)
Q06 = 

## 2. Realita, ambice a časový horizont

# Jakou úroveň ambice má v0.1 mít?  (doporučeno: B)
Q07 = 

# Jaký je tvrdý časový horizont do 0.1.0?  (doporučeno: C)
Q08 = 

# Jak dosáhnout cíle při maximálním rozsahu?  (doporučeno: B)
Q07b = 

# Souhlasíš s tímto členěním na milníky M0–M5?  (doporučeno: A)
Q09 = 

# Co se má řezat jako první, když se nestíhá?  (doporučeno: A)
Q10 = 

# Šířka, nebo hloubka?  (doporučeno: A)
Q11 = 

## 3. Licence, veřejnost a podoba repozitáře

# Jakou licenci bude mít náš kód?  (doporučeno: A)
Q12 = 

# Kdy bude repozitář veřejný?  (doporučeno: A)
Q13 = 

# Jak bude vypadat „co se změnilo" u každého commitu?  (doporučeno: A)
Q14 = 

# Které „signály života" a governance soubory chceme od začátku?  (doporučeno: A,B,C,D,F,G)
Q15 = 

## 4. Architektura a hranice systému

# Jak má vypadat Cargo workspace?  (doporučeno: A)
Q16 = 

# Jaký procesní model má mít 0.1?  (doporučeno: A)
Q17 = 

# Jaký má být model vláken a plánování?  (doporučeno: A)
Q18 = 

# Kdo vlastní event loop?  (doporučeno: A)
Q19 = 

# Jaké rozhraní pro vkládání (embedding) bude jádro mít?  (doporučeno: A)
Q20 = 

# Jak bude vypadat DOM v paměti?  (doporučeno: A)
Q21 = 

## 5. Apple Silicon, GPU a vlákna

# Přes jaké API mluvit s GPU?  (doporučeno: A)
Q22 = 

# Jak sladit přímý Metal s rasterizérem?  (doporučeno: C)
Q22b = 

# Čím rasterizovat vektory a text?  (doporučeno: A)
Q23 = 

# Budeme explicitně řídit QoS a P/E jádra?  (doporučeno: A)
Q24 = 

# Kolik „zero-copy" UMA integrace chceme už v 0.1?  (doporučeno: A)
Q25 = 

# Jaké platformy cílíme v 0.1?  (doporučeno: A)
Q26 = 

## 6. Paměťová strategie

# Jaký globální alokátor?  (doporučeno: A)
Q27 = 

# Jak budou fungovat areny?  (doporučeno: A)
Q28 = 

# Budeme mít tvrdý paměťový rozpočet na dokument?  (doporučeno: A)
Q29 = 

# Které cache a zálohy chceme už v 0.1?  (doporučeno: A)
Q30 = 

## 7. Síťová vrstva

# Na čem postavit HTTP?  (doporučeno: A)
Q31 = 

# Odkud brát důvěryhodné certifikáty?  (doporučeno: A)
Q32 = 

# HTTP/3 (QUIC) už v 0.1?  (doporučeno: A)
Q33 = 

# Jak řešit HTTP cache?  (doporučeno: A)
Q34 = 

# Cookies, proxy a DoH – v jakém rozsahu?  (doporučeno: A)
Q35 = 

## 8. HTML, DOM a CSS

# HTML parser: použít html5ever, nebo psát vlastní?  (doporučeno: A)
Q36 = 

# Potřebujeme pozice ve zdrojovém HTML (source spans) už v 0.1?  (doporučeno: A)
Q37 = 

# Jak postavit DOM?  (doporučeno: A)
Q38 = 

# CSS: kaskáda a výpočet stylů – postavit na čem?  (doporučeno: A)
Q39 = 

# Které CSS funkce musí 0.1 umět?  (doporučeno: A,B,C,D,E,F,G,I,J,K)
Q40 = 

# Selektorový engine: vlastní, nebo hotový?  (doporučeno: A)
Q41 = 

# Web components (Shadow DOM, custom elements) v 0.1?  (doporučeno: A)
Q42 = 

# Iframes v 0.1?  (doporučeno: A)
Q43 = 

# Formulářové prvky v 0.1?  (doporučeno: A)
Q44 = 

## 9. Layout engine

# Jak rozdělit práci na layoutu?  (doporučeno: A)
Q45 = 

# Tabulky v 0.1?  (doporučeno: A)
Q46 = 

# Floaty a clear v 0.1?  (doporučeno: A)
Q47 = 

# Inkrementální layout od začátku, nebo nejdřív plný přepočet?  (doporučeno: A)
Q48 = 

# Paralelizace layoutu?  (doporučeno: A)
Q49 = 

# Scroll kontexty, sticky a writing modes?  (doporučeno: A)
Q50 = 

## 10. Text, fonty a typografie

# Který shaping engine?  (doporučeno: A)
Q51 = 

# Odkud brát fonty a fallback?  (doporučeno: A)
Q52 = 

# Jak rasterizovat glyfy?  (doporučeno: A)
Q53 = 

# Jaký typografický cíl kvality?  (doporučeno: A)
Q54 = 

# Textový editing, kurzor, výběr a IME v 0.1?  (doporučeno: A)
Q55 = 

# Variabilní fonty, barevné fonty a emoji?  (doporučeno: A)
Q56 = 

## 11. Vykreslování, kompozice a vizuální kvalita

# Jaký model vykreslování?  (doporučeno: A)
Q57 = 

# Kdy vytvářet samostatné kompoziční vrstvy?  (doporučeno: A)
Q58 = 

# Které kvalitativní vlastnosti jsou pro nás priorita?  (doporučeno: A,B,D,E,F,H)
Q59 = 

# Které vizuální efekty musí 0.1 umět?  (doporučeno: A,B,C,D,G)
Q60 = 

# Canvas, obrázky, SVG a video v 0.1?  (doporučeno: A)
Q61 = 

## 12. JavaScript, WebAssembly a bindingy

# Který JavaScript engine?  (doporučeno: B)
Q62 = 

# Jak generovat DOM bindingy pro JS?  (doporučeno: A)
Q63 = 

# Jak řešit vztah JS GC a DOM?  (doporučeno: A)
Q64 = 

# WebAssembly v 0.1?  (doporučeno: A)
Q65 = 

# Moduly, workers a event loop JS?  (doporučeno: A)
Q66 = 

## 13. Web API surface a formuláře

# Která Web API musí 0.1 mít?  (doporučeno: A,B,C,D,E,F,G,H,I,J,K,L)
Q67 = 

# Storage nad rámec localStorage?  (doporučeno: A)
Q68 = 

# Integrace s platformou (soubory, schránka, oprávnění)?  (doporučeno: A)
Q69 = 

# Přístupnost (accessibility) v 0.1?  (doporučeno: A)
Q70 = 

## 14. Měření: jak prokážeme, že jsme dobří

# Kdy se Speedometer stane součástí brány?  (doporučeno: A)
Q71 = 

# Jaký konkrétní WPT cíl pro 0.1?  (doporučeno: B)
Q72 = 

# Které benchmarky a metriky budou součástí bran (gates)?  (doporučeno: B,C,D,E,F,G,H)
Q73 = 

# Proti čemu a jak budeme měřit?  (doporučeno: A)
Q74 = 

# Jak budeme publikovat výsledky?  (doporučeno: A)
Q75 = 

# Kde budeme hledat výkonnostní výhru?  (doporučeno: A,B,C,D)
Q76 = 

## 15. Bezpečnost, sandbox a soukromí

# Origin model, same-origin policy a CORS v 0.1?  (doporučeno: A)
Q77 = 

# Jaký sandbox a hardening v 0.1?  (doporučeno: A)
Q78 = 

# Content Security Policy v 0.1?  (doporučeno: A)
Q79 = 

# Které bezpečnostní praktiky budou povinné od prvního commitu?  (doporučeno: A,C,D)
Q80 = 

## 16. AI a agentní vrstva

# Kde je hranice mezi jádrem a AI?  (doporučeno: A)
Q81 = 

# V jakém formátu má jádro vydávat stránku pro AI?  (doporučeno: A)
Q82 = 

# Kolik AI chceme v 0.1?  (doporučeno: A)
Q83 = 

## 17. Dokumentace, CI a release

# Jaký rozsah češtiny v dokumentaci?  (doporučeno: A)
Q84 = 

# Jaký standard dokumentace v kódu?  (doporučeno: A)
Q85 = 

# Kde běžet CI?  (doporučeno: A)
Q86 = 

# Jak hlídat výkonnostní regrese?  (doporučeno: A)
Q87 = 

# Jaké release artefakty a signing?  (doporučeno: A)
Q88 = 

# Jak postavit dokumentační web?  (doporučeno: A)
Q89 = 

## 18. Proces: agenti, pluginy a definice hotového

# Jak má vypadat tým agentů?  (doporučeno: A)
Q90 = 

# Jak přísné mají být merge gates?  (doporučeno: A)
Q91 = 

# Které interní pluginy do harnessu máme postavit?  (doporučeno: A,B,C)
Q92 = 

# Jak využívat memory a graphify?  (doporučeno: A)
Q93 = 

# Co musí splnit 0.1.0, aby se vydala?  (doporučeno: A,B,C,D,E,F)
Q94 = 

# Co se má stát hned po tvém vyplnění briefingu?  (doporučeno: A)
Q95 = 

# Co má být hotové za prvních 14 dní (M0)?  (doporučeno: A)
Q96 = 

# Jak často komunikovat pokrok?  (doporučeno: A)
Q97 = 
