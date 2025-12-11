## Lancer rapidement (mock)
```bash
cargo run
```
Le serveur écoute sur `ws://127.0.0.1:8080` et génère des prix aléatoires.

## Lancer avec PostgreSQL
1) Préparer la base (même schéma que TD1) :
```bash
createdb stockdb
psql stockdb < schema.sql
```
2) Exporter l’URL :
```bash
# Unix
export DATABASE_URL="postgresql://user:password@localhost/stockdb"
# PowerShell
$env:DATABASE_URL = 'postgresql://user:password@localhost/stockdb'
```
3) Démarrer :
```bash
cargo run
```
Le serveur poll `stock_prices` toutes les 5s et diffuse les derniers prix par symbole/source.

```bash
cd "rust-td 2"
python -m http.server 8000
```
Puis aller sur http://127.0.0.1:8000/client.html

## Tests
```bash
cargo test
```
