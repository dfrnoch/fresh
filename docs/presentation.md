---
marp: true
theme: default
paginate: true
backgroundColor: "#1f2023"
color: "#ffffff"
style: |
  section {
    align-items: center;
    justify-content: center;
    display: flex;
  }
  h1, h2, h3, h4, h5, h6 {
    color: #ffffff;
  }
  code {
    background-color: rgba(255,255,255,0.1);
    color: #ffffff;
    padding: 2px 6px;
    border-radius: 4px;
  }
  .custom-icon {
    max-width: 300px;
    max-height: 300px;
    background-color: transparent;
  }
---

# Fresh
### Klauzurní práce
#### David Frnoch - 3C
---

# Úvod

- Fresh je serverová/klientská chatovací aplikace napsaná v jazyce Rust
- Používá TCP sockety a JSON zprávy pro komunikaci
- Umožňuje soukromé zprávy, skupinové chaty a příkazy pro operátory

---

# Architektura

- Server
  - Zpracovává připojení klientů
  - Spravuje uživatele a chatovací místnosti
  - Směruje zprávy mezi klienty

- Klient
  - Připojuje se k serveru
  - Zpracovává příchozí zprávy
  - Zajišťuje vstup od uživatele

- Společné
  - Sdílený kód mezi klientem a serverem (protokol, modely)

---

# Design

- Modulární: oddělené moduly pro vstup, zprávy, připojení atd.
- Funkcionální: funkce jsou malé a zaměřené na jednu věc
- Zaměřený na data: datové struktury jsou navrženy pro daný účel
- Paralelní zpracování: server zpracovává klienty v paralelních vláknech

---

# Výhody

- Spolehlivost: bezpečnost paměti v Rustu snižuje chyby
- Výkon: zkompilované binárky jsou rychlé
- Udržovatelnost: přísný modulární systém udržuje kód organizovaný

---

# Možnosti zlepšení

- Testování: velmi málo kódu má automatizované testy
- Ergonomie: některé části kódu by mohly být více idiomatičtější pro Rust
- Škálovatelnost: architektura může mít problémy při velkém měřítku

---

# Ukázka aplikace


---

# Děkuji za pozornost


