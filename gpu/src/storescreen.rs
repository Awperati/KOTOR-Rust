//! A shop: `store.gui`, as the reference video has it at 50:10 and 50:20. One list at a time, Buy or Sell, with a button
//! to change between them. A click chooses a row and shows its words, and the button at the foot trades one of it.
//! The rules are `kotor_game::store`.
//!
//! The question over a dear item is in the video and not in the talk table, so its words are the video's.
//! **Assumed:** it comes for anything over the 150 it names, and not for selling, where I saw none. **Assumed:** the list
//! keeps the file's row height, as every list here does, where the video's rows are about 13 units: `docs/MENU_REFERENCE.md`.

use super::Stage;
use kotor_game::{CannotTrade, ObjId, StoreLine};
use kotor_ui::{MenuView, Row};

const STR_SELL: u32 = 32130;
const STR_BUY: u32 = 32132;
const STR_SHOW_BUY: u32 = 41937;
const STR_SHOW_SELL: u32 = 41938;
const STR_ITEM_COST: u32 = 41943;
const STR_ITEM_VALUE: u32 = 41945;
const BUY_LIST: &str = "LB_SHOPITEMS";
const SELL_LIST: &str = "LB_INVITEMS";
const ASK_ABOVE: i32 = 150;
const QUESTION: &str = "This item costs more than 150 credits. Do you still wish to buy?";
const TOO_DEAR: &str = "You do not have enough credits.";
const QUESTION_RISE: i32 = 10;

/// What the shop's screen remembers.
#[derive(Clone, Copy, Default)]
pub(super) struct Shop {
    /// The Sell list is up.
    selling: bool,
    chosen: Option<ObjId>,
    /// The question is up, about this.
    asking: Option<ObjId>,
}

impl Stage {
    /// The list that is up. A store that only buys or only sells has only that one.
    fn shop_selling(&self) -> bool {
        let store = self.game.world.store.as_ref();
        self.menus.shop.selling || store.is_some_and(|s| !s.can_buy)
    }

    fn shop_lines(&mut self) -> Vec<StoreLine> {
        if self.shop_selling() {
            self.game.world.sell_lines()
        } else {
            self.game.world.store_lines()
        }
    }

    pub(super) fn store_view(&mut self) -> MenuView {
        let mut v = MenuView::new("store");
        let selling = self.shop_selling();
        let lines = self.shop_lines();
        let chosen = self.menus.shop.chosen;
        let rows: Vec<Row> = lines
            .iter()
            .filter_map(|line| {
                let view = self.game.world.item_view(line.item)?;
                let mut row = self.item_row(&view, false);
                row.selected = chosen == Some(line.item);
                Some(row)
            })
            .collect();
        let (shown, hidden) = if selling {
            (SELL_LIST, BUY_LIST)
        } else {
            (BUY_LIST, SELL_LIST)
        };
        v.lists.insert(shown.to_string(), (rows, self.menus.first));
        v.hide(&[hidden]);
        let word =
            |s: &mut Self, sell: u32, buy: u32| s.ui.strref(if selling { sell } else { buy });
        v.text("LBL_BUYSELL", word(self, STR_SELL, STR_BUY));
        v.text("BTN_Accept", word(self, STR_SELL, STR_BUY));
        v.text("BTN_Examine", word(self, STR_SHOW_BUY, STR_SHOW_SELL));
        v.text("LBL_COST", word(self, STR_ITEM_VALUE, STR_ITEM_COST));
        v.text("LBL_CREDITS_VALUE", self.game.world.gold.to_string());

        let line = lines.iter().find(|l| Some(l.item) == chosen);
        let view = line.and_then(|l| self.game.world.item_view(l.item));
        v.text(
            "LBL_COST_VALUE",
            line.map_or(String::new(), |l| l.price.to_string()),
        );
        v.text(
            "LBL_STOCK_VALUE",
            view.as_ref().map_or(String::new(), |i| i.stack.to_string()),
        );
        v.text(
            "LB_DESCRIPTION",
            view.map_or(String::new(), |i| i.description),
        );
        if line.is_none() {
            v.inert(&["BTN_Accept"]);
        }
        let store = self.game.world.store.as_ref();
        if store.is_some_and(|s| !s.can_buy || !s.can_sell) {
            v.inert(&["BTN_Examine"]);
        }
        if self.menus.shop.asking.is_some() {
            let mut d = MenuView::new("confirm");
            d.text("LB_MESSAGE", QUESTION);
            // As on the party screen: the file's font runs off the box.
            d.font("LB_MESSAGE", "fnt_d16x16");
            d.patches.entry("LB_MESSAGE".to_string()).or_default().rise = QUESTION_RISE;
            d.hover = self.menus.hover.clone();
            v.dialog = Some(Box::new(d));
        }
        v
    }

    pub(super) fn store_row(&mut self, row: usize) {
        self.menus.shop.chosen = self.shop_lines().get(row).map(|l| l.item);
    }

    fn close_shop(&mut self) {
        self.game.world.close_store();
        self.menus.shop = Shop::default();
        self.menus.first = 0;
        self.menus.hover = None;
    }

    /// Esc: the question goes first, then the shop.
    pub(super) fn store_escape(&mut self) {
        if self.menus.shop.asking.take().is_none() {
            self.close_shop();
        }
    }

    /// Trade what is chosen. The list may be shorter after, so what is chosen and the first row shown are looked at again.
    fn trade(&mut self, item: ObjId) {
        let w = &mut self.game.world;
        let done = if self.menus.shop.selling || w.store.as_ref().is_some_and(|s| !s.can_buy) {
            w.store_sell(item)
        } else {
            w.store_buy(item)
        };
        if done == Err(CannotTrade::TooDear) {
            self.menus.message = TOO_DEAR.to_string();
        }
        let lines = self.shop_lines();
        if !lines.iter().any(|l| l.item == item) {
            self.menus.shop.chosen = None;
        }
        self.menus.first = self.menus.first.min(lines.len().saturating_sub(1));
    }

    pub(super) fn store_press(&mut self, tag: &str) {
        if let Some(item) = self.menus.shop.asking.take() {
            if tag == "BTN_OK" {
                self.trade(item);
            }
            self.menus.hover = None;
            return;
        }
        match tag {
            "BTN_Accept" => self.store_accept(),
            "BTN_Examine" => {
                let both =
                    (self.game.world.store.as_ref()).is_some_and(|s| s.can_buy && s.can_sell);
                if both {
                    self.menus.shop.selling = !self.menus.shop.selling;
                    self.menus.shop.chosen = None;
                    self.menus.first = 0;
                }
            }
            "BTN_Cancel" => self.close_shop(),
            _ => {}
        }
    }

    /// The button at the foot, or Enter.
    pub(super) fn store_accept(&mut self) {
        if self.menus.shop.asking.is_some() {
            return self.store_press("BTN_OK");
        }
        let selling = self.shop_selling();
        let lines = self.shop_lines();
        let chosen = self.menus.shop.chosen;
        let Some(line) = lines.iter().find(|l| Some(l.item) == chosen) else {
            return;
        };
        let affordable = line.price <= self.game.world.gold;
        if !selling && affordable && line.price > ASK_ABOVE {
            self.menus.shop.asking = Some(line.item);
            self.menus.hover = None;
        } else {
            self.trade(line.item);
        }
    }
}
