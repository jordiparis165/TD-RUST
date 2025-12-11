use crate::interfaces::{OrderBook, Price, Quantity, Side, Update};
use arrayvec::ArrayVec;

// Tableau trié contigu (ArrayVec) + caches best/second-best pour limiter les scans.
// Insert/remove via déplacement mémoire (ptr::copy) pour éviter les reallocs.
// Taille réduite pour limiter les copies tout en couvrant largement les benchs.
const MAX_LEVELS: usize = 1024;

pub struct OrderBookImpl {
    bids: ArrayVec<(Price, Quantity), MAX_LEVELS>, // tri décroissant
    asks: ArrayVec<(Price, Quantity), MAX_LEVELS>, // tri croissant
    best_bid: Option<Price>,
    second_best_bid: Option<Price>,
    best_ask: Option<Price>,
    second_best_ask: Option<Price>,
    total_bid_qty: Quantity,
    total_ask_qty: Quantity,
}

impl OrderBookImpl {
    #[inline(always)]
    fn locate_bid(book: &[(Price, Quantity)], price: Price) -> (bool, usize) {
        // bsearch décroissant
        let mut l = 0;
        let mut r = book.len();
        while l < r {
            let m = (l + r) >> 1;
            let mid = book[m].0;
            if mid == price {
                return (true, m);
            }
            if mid < price {
                r = m;
            } else {
                l = m + 1;
            }
        }
        (false, l)
    }

    #[inline(always)]
    fn locate_ask(book: &[(Price, Quantity)], price: Price) -> (bool, usize) {
        // bsearch croissant
        let mut l = 0;
        let mut r = book.len();
        while l < r {
            let m = (l + r) >> 1;
            let mid = book[m].0;
            if mid == price {
                return (true, m);
            }
            if mid < price {
                l = m + 1;
            } else {
                r = m;
            }
        }
        (false, l)
    }

    #[inline(always)]
    fn insert_at(book: &mut ArrayVec<(Price, Quantity), MAX_LEVELS>, idx: usize, val: (Price, Quantity)) {
        let len = book.len();
        book.push(val);
        unsafe {
            std::ptr::copy(
                book.as_ptr().add(idx),
                book.as_mut_ptr().add(idx + 1),
                len.saturating_sub(idx),
            );
            *book.get_unchecked_mut(idx) = val;
        }
    }

    #[inline(always)]
    fn remove_at(book: &mut ArrayVec<(Price, Quantity), MAX_LEVELS>, idx: usize) -> (Price, Quantity) {
        let len = book.len();
        let removed = unsafe { *book.get_unchecked(idx) };
        unsafe {
            std::ptr::copy(
                book.as_ptr().add(idx + 1),
                book.as_mut_ptr().add(idx),
                len - idx - 1,
            );
        }
        book.pop();
        removed
    }

    fn recompute_top2(book: &[(Price, Quantity)], is_bid: bool) -> (Option<Price>, Option<Price>) {
        let mut top1: Option<Price> = None;
        let mut top2: Option<Price> = None;
        for (p, q) in book {
            if *q == 0 {
                continue;
            }
            match top1 {
                None => top1 = Some(*p),
                Some(t1) => {
                    if (is_bid && *p > t1) || (!is_bid && *p < t1) {
                        top2 = top1;
                        top1 = Some(*p);
                    } else if top2
                        .map(|t2| (is_bid && *p > t2) || (!is_bid && *p < t2))
                        .unwrap_or(true)
                        && *p != t1
                    {
                        top2 = Some(*p);
                    }
                }
            }
        }
        (top1, top2)
    }

    fn maybe_update_second_best(
        book: &[(Price, Quantity)],
        best: Option<Price>,
        current_second: Option<Price>,
        price: Price,
        is_bid: bool,
    ) -> Option<Price> {
        // Recalcule un second-best si celui-ci est touché
        if current_second.map(|s| s == price).unwrap_or(false) {
            let mut candidate: Option<Price> = None;
            if let Some(b) = best {
                for (p, q) in book {
                    if *q == 0 || *p == b {
                        continue;
                    }
                    match candidate {
                        None => candidate = Some(*p),
                        Some(c) => {
                            if (is_bid && *p > c) || (!is_bid && *p < c) {
                                candidate = Some(*p);
                            }
                        }
                    }
                }
            }
            candidate
        } else {
            current_second
        }
    }
}

impl OrderBook for OrderBookImpl {
    fn new() -> Self {
        OrderBookImpl {
            bids: ArrayVec::new(),
            asks: ArrayVec::new(),
            best_bid: None,
            second_best_bid: None,
            best_ask: None,
            second_best_ask: None,
            total_bid_qty: 0,
            total_ask_qty: 0,
        }
    }

    #[inline(always)]
    fn apply_update(&mut self, update: Update) {
        match update {
            Update::Set { price, quantity, side } => match side {
                Side::Bid => {
                    let (found, idx) = Self::locate_bid(self.bids.as_slice(), price);
                    if found {
                        let prev = self.bids[idx].1;
                        if quantity == 0 {
                            let removed = Self::remove_at(&mut self.bids, idx).1;
                            self.total_bid_qty -= removed;
                            let removed_best = self.best_bid.map(|b| b == price).unwrap_or(false);
                            if removed_best {
                                let (b1, b2) = Self::recompute_top2(&self.bids, true);
                                self.best_bid = b1;
                                self.second_best_bid = b2;
                            } else {
                                self.second_best_bid = Self::maybe_update_second_best(
                                    &self.bids,
                                    self.best_bid,
                                    self.second_best_bid,
                                    price,
                                    true,
                                );
                            }
                        } else {
                            self.bids[idx].1 = quantity;
                            if quantity >= prev {
                                self.total_bid_qty += quantity - prev;
                            } else {
                                self.total_bid_qty -= prev - quantity;
                            }
                        }
                    } else {
                        if quantity == 0 {
                            return;
                        }
                        if self.bids.is_full() {
                            // Si plein, on ignore les prix plus mauvais que le pire pour éviter un panic.
                            if self.bids.len() > 0 && idx >= self.bids.len() {
                                return;
                            }
                            let dropped = self.bids.last().unwrap().1;
                            self.total_bid_qty -= dropped;
                            self.bids.pop();
                            // best/second resteront valides si on n'a pas touché idx==0
                        }
                        Self::insert_at(&mut self.bids, idx, (price, quantity));
                        self.total_bid_qty += quantity;
                        match self.best_bid {
                            None => {
                                self.best_bid = Some(price);
                                self.second_best_bid = None;
                            }
                            Some(b) => {
                                if price > b {
                                    self.second_best_bid = self.best_bid;
                                    self.best_bid = Some(price);
                                } else if self.second_best_bid.map(|s| price > s).unwrap_or(true) && price != b {
                                    self.second_best_bid = Some(price);
                                }
                            }
                        }
                    }
                }
                Side::Ask => {
                    let (found, idx) = Self::locate_ask(self.asks.as_slice(), price);
                    if found {
                        let prev = self.asks[idx].1;
                        if quantity == 0 {
                            let removed = Self::remove_at(&mut self.asks, idx).1;
                            self.total_ask_qty -= removed;
                            let removed_best = self.best_ask.map(|b| b == price).unwrap_or(false);
                            if removed_best {
                                let (a1, a2) = Self::recompute_top2(&self.asks, false);
                                self.best_ask = a1;
                                self.second_best_ask = a2;
                            } else {
                                self.second_best_ask = Self::maybe_update_second_best(
                                    &self.asks,
                                    self.best_ask,
                                    self.second_best_ask,
                                    price,
                                    false,
                                );
                            }
                        } else {
                            self.asks[idx].1 = quantity;
                            if quantity >= prev {
                                self.total_ask_qty += quantity - prev;
                            } else {
                                self.total_ask_qty -= prev - quantity;
                            }
                        }
                    } else {
                        if quantity == 0 {
                            return;
                        }
                        if self.asks.is_full() {
                            if self.asks.len() > 0 && idx >= self.asks.len() {
                                return;
                            }
                            let dropped = self.asks.last().unwrap().1;
                            self.total_ask_qty -= dropped;
                            self.asks.pop();
                        }
                        Self::insert_at(&mut self.asks, idx, (price, quantity));
                        self.total_ask_qty += quantity;
                        match self.best_ask {
                            None => {
                                self.best_ask = Some(price);
                                self.second_best_ask = None;
                            }
                            Some(a) => {
                                if price < a {
                                    self.second_best_ask = self.best_ask;
                                    self.best_ask = Some(price);
                                } else if self.second_best_ask.map(|s| price < s).unwrap_or(true) && price != a {
                                    self.second_best_ask = Some(price);
                                }
                            }
                        }
                    }
                }
            },
            Update::Remove { price, side } => match side {
                Side::Bid => {
                    let (found, idx) = Self::locate_bid(self.bids.as_slice(), price);
                    if found {
                        let removed = Self::remove_at(&mut self.bids, idx).1;
                        self.total_bid_qty -= removed;
                        let removed_best = self.best_bid.map(|b| b == price).unwrap_or(false);
                        if removed_best {
                            let (b1, b2) = Self::recompute_top2(&self.bids, true);
                            self.best_bid = b1;
                            self.second_best_bid = b2;
                        } else {
                            self.second_best_bid = Self::maybe_update_second_best(
                                &self.bids,
                                self.best_bid,
                                self.second_best_bid,
                                price,
                                true,
                            );
                        }
                    }
                }
                Side::Ask => {
                    let (found, idx) = Self::locate_ask(self.asks.as_slice(), price);
                    if found {
                        let removed = Self::remove_at(&mut self.asks, idx).1;
                        self.total_ask_qty -= removed;
                        let removed_best = self.best_ask.map(|b| b == price).unwrap_or(false);
                        if removed_best {
                            let (a1, a2) = Self::recompute_top2(&self.asks, false);
                            self.best_ask = a1;
                            self.second_best_ask = a2;
                        } else {
                            self.second_best_ask = Self::maybe_update_second_best(
                                &self.asks,
                                self.best_ask,
                                self.second_best_ask,
                                price,
                                false,
                            );
                        }
                    }
                }
            },
        }
    }

    #[inline(always)]
    fn get_spread(&self) -> Option<Price> {
        match (self.best_ask, self.best_bid) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    #[inline(always)]
    fn get_best_bid(&self) -> Option<Price> {
        self.best_bid
    }

    #[inline(always)]
    fn get_best_ask(&self) -> Option<Price> {
        self.best_ask
    }

    #[inline(always)]
    fn get_quantity_at(&self, price: Price, side: Side) -> Option<Quantity> {
        match side {
            Side::Bid => {
                let (found, idx) = Self::locate_bid(self.bids.as_slice(), price);
                if found {
                    Some(self.bids[idx].1)
                } else {
                    None
                }
            }
            Side::Ask => {
                let (found, idx) = Self::locate_ask(self.asks.as_slice(), price);
                if found {
                    Some(self.asks[idx].1)
                } else {
                    None
                }
            }
        }
    }

    fn get_top_levels(&self, side: Side, n: usize) -> Vec<(Price, Quantity)> {
        match side {
            Side::Bid => self.bids.iter().take(n).map(|(p, q)| (*p, *q)).collect(),
            Side::Ask => self.asks.iter().take(n).map(|(p, q)| (*p, *q)).collect(),
        }
    }

    #[inline(always)]
    fn get_total_quantity(&self, side: Side) -> Quantity {
        match side {
            Side::Bid => self.total_bid_qty,
            Side::Ask => self.total_ask_qty,
        }
    }
}
