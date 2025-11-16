===============================
 Loglyzer - Rust Log Analyzer
===============================

Ce projet inclut deux exécutables :
- loglyzer       : analyse les fichiers de logs
- generate_logs  : génère de grands fichiers de logs de test


--------------------------------------------------
Lancer l'analyseur (loglyzer)
--------------------------------------------------

Comme il y a deux binaires, il faut préciser lequel lancer :

    cargo run --bin loglyzer -- <fichier> [options]

Exemple :

    cargo run --bin loglyzer -- sample.log

Options que vous voulez :
    --format text|json|csv
    --errors-only
    --search <texte>
    --top <N>
    --verbose
    --output <fichier>
    --parallel


Exemples d'utilisation :

    # Analyse simple
    cargo run --bin loglyzer -- sample.log

    # Mode verbeux
    cargo run --bin loglyzer -- sample.log -v

    # Filtrer les erreurs
    cargo run --bin loglyzer -- sample.log --errors-only

    # Top 5 erreurs 
    cargo run --bin loglyzer -- sample.log --errors-only --search database --top 5

    # Sortie JSON
    cargo run --bin loglyzer -- sample.log --format json

    # Export CSV dans un fichier
    cargo run --bin loglyzer -- sample.log --format csv --output stats.csv

    # Mode parallèle forcé
    cargo run --bin loglyzer -- big.log --parallel -v



--------------------------------------------------
 Génération d'un gros fichier de logs
--------------------------------------------------

Le projet inclut un deuxième programme : generate_logs.

Lancer la génération :

    cargo run --bin generate_logs

Cela crée un fichier "generated.log" contenant 100 000 lignes.

Vous pouvez aussi spécifier :
- le nombre de lignes
- le nom du fichier

Exemples :

    cargo run --bin generate_logs -- 200000 big.log
    cargo run --bin generate_logs -- 500000 test.log


--------------------------------------------------
Tester le mode parallèle
--------------------------------------------------

    cargo run --bin generate_logs -- 300000 big.log

Puis :

    cargo run --bin loglyzer -- big.log -v
    cargo run --bin loglyzer -- big.log -v --parallel


--------------------------------------------------
Structure du projet
--------------------------------------------------

src/
    main.rs              -> analyseur loglyzer
    bin/generate_logs.rs -> générateur de logs
Cargo.toml
sample.log



