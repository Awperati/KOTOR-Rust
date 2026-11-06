//! What a nameplate needs to know of an object: whether it could be clicked, and what its label says.

use crate::world::Kind;
use crate::{ObjId, World};
use kotor_core::ResType;

/// What a label on something that is not a living creature says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectLabel {
    /// A container or remains with something in them, or a container not looked in yet.
    Loot,
    /// A container that was looked in and holds nothing.
    Looted,
    /// A terminal, a workbench or anything else that is used.
    Usable,
    Door,
}

impl World {
    /// Whether a click on the object's model would do something. As `pick` has it.
    pub fn clickable(&self, id: ObjId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable)
                && o.id != self.pc
                && o.useable
                && (!o.dead || self.lootable(id))
        })
    }

    /// What the label of a thing says. Scenery, script objects and the living have none.
    pub fn object_label(&mut self, id: ObjId) -> Option<ObjectLabel> {
        let o = self.objects.get(&id)?;
        if o.name.trim().is_empty() {
            return None;
        }
        match o.kind {
            Kind::Creature if o.dead => self.lootable(id).then_some(ObjectLabel::Loot),
            Kind::Door if o.useable && !o.open => Some(ObjectLabel::Door),
            Kind::Placeable if o.useable => {
                let (template, holds) = (o.template.clone(), !o.inventory.is_empty());
                if holds {
                    return Some(ObjectLabel::Loot);
                }
                if !self.template_has_inventory(&template) {
                    return Some(ObjectLabel::Usable);
                }
                // **Assumed:** one not looked in yet has something: its scripts fill it when it is opened.
                Some(if self.filled.contains(&id) {
                    ObjectLabel::Looted
                } else {
                    ObjectLabel::Loot
                })
            }
            _ => None,
        }
    }

    fn template_has_inventory(&mut self, template: &str) -> bool {
        if let Some(has) = self.inventory_templates.get(template) {
            return *has;
        }
        let has = self
            .inst
            .read(template, ResType(2044))
            .ok()
            .and_then(|b| kotor_gff::Gff::parse(&b).ok())
            .is_some_and(|g| g.root.int("HasInventory").is_some_and(|h| h != 0));
        self.inventory_templates.insert(template.to_string(), has);
        has
    }

    /// Whether the player could see the spot, at a throw of the same test as a fight's.
    pub fn in_plain_sight(&self, id: ObjId) -> bool {
        let (Some(a), Some(b)) = (self.objects.get(&self.pc), self.objects.get(&id)) else {
            return false;
        };
        self.clear_line(a.position, b.position)
    }
}
