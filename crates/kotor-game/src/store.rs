//! Shops: a store's `.utm` holds what it sells, its mark-ups, and whether it buys. `OpenStore` opens one; the screen is
//! the viewer's, and the rules are here.
//!
//! **Read from the install:** `MarkUp`, `MarkDown`, `BuySellFlag`, `ItemList` with `InventoryRes` and `Infinite`, and the
//! items' `Cost` and `AddCost`. An item is worth its `Cost`, which already holds `AddCost` (see `item_value`), or its
//! `AddCost` where a template left `Cost` at 0. **Assumed, not compared with the game running:** a price is the worth
//! times the percent over 100, at least 1, and an item is sold one at a time. `BuySellFlag` bit 1 lets the party buy and bit 2 lets it sell. Stock
//! comes back when a module is entered again. Plot items cannot be sold.

use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;

pub(crate) const STORE_FILE: ResType = ResType(2051);
const ITEM_FILE: ResType = ResType(2025);

/// A store that is open: `World::store`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreOpen {
    pub store: ObjId,
    /// Percent of an item's value the party pays.
    pub mark_up: i32,
    /// Percent of an item's value the party is paid.
    pub mark_down: i32,
    pub can_buy: bool,
    pub can_sell: bool,
}

/// One row of a store's screen. `World::item_view` has the rest of what a screen shows about the item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreLine {
    pub item: ObjId,
    pub name: String,
    pub price: i32,
    /// It never runs out.
    pub infinite: bool,
}

/// Why a sale did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotTrade {
    NoStoreOpen,
    NotForSale,
    TooDear,
    WillNotBuy,
    Plot,
}

impl World {
    /// `OpenStore`: `bonus_up` and `bonus_down` are added to the store's own percentages.
    pub fn open_store(&mut self, store: ObjId, bonus_up: i32, bonus_down: i32) {
        if self.law_refuses(store) {
            self.events
                .push("The shop is shut: the law is after you".to_string());
            return;
        }
        let Some(template) = self
            .object(store)
            .filter(|o| o.kind == Kind::Store)
            .map(|o| o.template.clone())
        else {
            self.errors
                .push(format!("OpenStore: {store} is not a store"));
            return;
        };
        let Some(utm) = self
            .inst
            .read(&template, STORE_FILE)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            self.errors
                .push(format!("OpenStore: no store file {template}"));
            return;
        };
        if self.filled.insert(store) {
            let stock: Vec<(String, bool)> = utm
                .root
                .list("ItemList")
                .unwrap_or_default()
                .iter()
                .filter_map(|i| {
                    let res = i.str("InventoryRes")?.to_string();
                    Some((res, i.int("Infinite").is_some_and(|n| n != 0)))
                })
                .collect();
            for (res, infinite) in stock {
                let item = self.spawn_item(&res);
                if infinite {
                    self.store_infinite.insert(item);
                }
                if let Some(o) = self.object_mut(store) {
                    o.inventory.push(item);
                }
            }
        }
        let flag = utm.root.int("BuySellFlag").unwrap_or(3);
        self.store = Some(StoreOpen {
            store,
            mark_up: utm.root.int("MarkUp").unwrap_or(100) as i32 + bonus_up,
            mark_down: utm.root.int("MarkDown").unwrap_or(100) as i32 + bonus_down,
            can_buy: flag & 1 != 0,
            can_sell: flag & 2 != 0,
        });
        self.last_opened_by = self.pc;
        self.run_event(store, "OnOpenStore");
    }

    /// What an item is worth before a store's percentages.
    pub fn item_value(&mut self, item: ObjId) -> i32 {
        let Some(template) = self.object(item).map(|o| o.template.clone()) else {
            return 0;
        };
        let Some(g) = self
            .inst
            .read(&template, ITEM_FILE)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            return 0;
        };
        let (cost, add) = (
            g.root.int("Cost").unwrap_or(0),
            g.root.int("AddCost").unwrap_or(0),
        );
        // `Cost` already holds `AddCost`: of the 557 item templates in the BIFs, 438 set `AddCost`, 367 of them with
        // `Cost` the same and 35 more; the other 36 left `Cost` at 0. Adding them doubled the price.
        cost.max(add) as i32
    }

    fn line(&mut self, item: ObjId, percent: i32) -> StoreLine {
        let price = (self.item_value(item) * percent / 100).max(1);
        let name = self.object(item).map_or(String::new(), |o| {
            if o.name.is_empty() {
                o.template.clone()
            } else {
                o.name.clone()
            }
        });
        StoreLine {
            item,
            name,
            price,
            infinite: self.store_infinite.contains(&item),
        }
    }

    /// What the open store sells.
    pub fn store_lines(&mut self) -> Vec<StoreLine> {
        let Some(s) = self.store.clone() else {
            return Vec::new();
        };
        let stock = self
            .object(s.store)
            .map_or(Vec::new(), |o| o.inventory.clone());
        stock.into_iter().map(|i| self.line(i, s.mark_up)).collect()
    }

    /// What the party could sell to the open store: what it carries and does not wear, and nothing plot.
    pub fn sell_lines(&mut self) -> Vec<StoreLine> {
        let Some(s) = self.store.clone() else {
            return Vec::new();
        };
        let mut carried = self.carried(self.pc);
        carried.retain(|i| self.object(*i).is_some_and(|o| !o.plot));
        carried
            .into_iter()
            .map(|i| self.line(i, s.mark_down))
            .collect()
    }

    /// Buy one of an item in the open store's stock.
    pub fn store_buy(&mut self, item: ObjId) -> Result<(), CannotTrade> {
        let s = self.store.clone().ok_or(CannotTrade::NoStoreOpen)?;
        if !s.can_buy
            || !self
                .object(s.store)
                .is_some_and(|o| o.inventory.contains(&item))
        {
            return Err(CannotTrade::NotForSale);
        }
        let price = self.line(item, s.mark_up).price;
        if price > self.gold {
            return Err(CannotTrade::TooDear);
        }
        self.gold -= price;
        let bought = if self.store_infinite.contains(&item) {
            let template = self
                .object(item)
                .map_or(String::new(), |o| o.template.clone());
            self.spawn_item(&template)
        } else {
            if let Some(o) = self.object_mut(s.store) {
                o.inventory.retain(|i| *i != item);
            }
            item
        };
        let pc = self.pc;
        self.carry_acquired(pc, bought);
        Ok(())
    }

    /// Sell one of something the party carries to the open store.
    pub fn store_sell(&mut self, item: ObjId) -> Result<(), CannotTrade> {
        let s = self.store.clone().ok_or(CannotTrade::NoStoreOpen)?;
        if !s.can_sell {
            return Err(CannotTrade::WillNotBuy);
        }
        let pc = self.pc;
        if !self.carried(pc).contains(&item) {
            return Err(CannotTrade::NotForSale);
        }
        let Some((plot, stack, template)) = self
            .object(item)
            .map(|o| (o.plot, o.stack, o.template.clone()))
        else {
            return Err(CannotTrade::NotForSale);
        };
        if plot {
            return Err(CannotTrade::Plot);
        }
        self.gold += self.line(item, s.mark_down).price;
        let sold = if stack > 1 {
            if let Some(o) = self.object_mut(item) {
                o.stack -= 1;
            }
            self.spawn_item(&template)
        } else {
            if let Some(o) = self.object_mut(pc) {
                o.inventory.retain(|i| *i != item);
            }
            item
        };
        if let Some(o) = self.object_mut(s.store) {
            o.inventory.push(sold);
        }
        Ok(())
    }

    /// Close the open store.
    pub fn close_store(&mut self) {
        self.store = None;
    }
}
