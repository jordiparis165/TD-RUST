## Implémentation de l'order book (`src/orderbook.rs`)
- J'ai choisi un stockage contigu avec `ArrayVec<(Price, Quantity), 1024>` : bids triés décroissant, asks croissants, sans réallocation après l'init.
- Insertions/suppressions faites via `std::ptr::copy` pour décaler les éléments sans allocation supplémentaire.
- Je garde en cache `best` et `second_best` de chaque côté ; je ne recalcule tout que si le best disparaît, sinon j'ajuste seulement ce qui change.
- Les totaux `total_bid_qty` et `total_ask_qty` sont mis à jour en O(1).
- Recherche binaire pour trouver un prix ; `get_best_*`, `get_spread`, `get_total_quantity`, `get_quantity_at` restent O(1) grâce aux caches et à l'accès direct, les mises à jour sont O(n) à cause du décalage contigu.

## Benchmarks (`src/benchmarks.rs`)
- J'ai mesuré avec `Instant` en lots (`BATCH_SIZE` 10_000, `UPDATE_BATCH_SIZE` 100_000) pour limiter l'effet de la granularité de l'horloge Windows.
- Échauffement au début pour remplir le carnet avant de chronométrer.
- Benchmarks séparés pour `apply_update`, `get_spread`, `get_best_bid`, `get_best_ask` et les lectures aléatoires (`get_quantity_at`), avec moyennes et percentiles P50/P95/P99 sur les updates.
- Affichage formaté : nombre total d'opérations et temps moyens par opération (ns) pour chaque groupe.

## Dépendances (`Cargo.toml`)
- Ajout de `arrayvec = "0.7"` pour le stockage contigu.
- Ajout de `rustc-hash = "1.1"` (pas utilisé pour l'instant, gardé en réserve).
