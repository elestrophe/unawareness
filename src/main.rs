use iced::{
    button, pick_list, Application, Button, Checkbox, Column, Command, Element as IcedElement,
    PickList, Row, Settings, Text,
};
use minidom::{quick_xml::Reader, Element, NSChoice};
use regex::Regex;
use std::collections::{HashMap,HashSet};
use std::fmt;
use std::fs::{create_dir_all, File};
use std::io::BufReader;
use thiserror::Error;

#[derive(Debug, Error)]
enum MainError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Minidom(#[from] minidom::Error),
    #[error("Missing variable `{0}`")]
    DotEnv(String, #[source] dotenv::Error),
    #[error("Malformed necrodancer.xml: {0}")]
    BadNecroXML(String),
    #[error(transparent)]
    Iced(#[from] iced::Error),
}

#[derive(Debug)]
struct Item {
    id: String,
    name: String,
    slot: Slot,
    image: String,
}

#[derive(Debug, Clone)]
struct Character {
    id: String,
    name: String,
    items: Vec<String>,
    curses: HashSet<Slot>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Slot {
    Shovel,
    Weapon,
    Head,
    Feet,
    Body,
    Ring,
    Spell,
    Torch,
    Action,
    Misc,
    Other,
}

impl Slot {
    fn all() -> Vec<Self> {
        use Slot::*;
        vec![
            Shovel, Weapon, Body, Head, Feet, Torch, Ring, Spell, Action, Misc, Other,
        ]
    }
}

impl std::str::FromStr for Slot {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Slot::*;
        Ok(match s {
            "shovel" => Shovel,
            "weapon" => Weapon,
            "head" => Head,
            "feet" => Feet,
            "body" => Body,
            "ring" => Ring,
            "spell" => Spell,
            "torch" => Torch,
            "action" => Action,
            "misc" => Misc,
            "hud" | "bomb" => Other,
            _ => return Err(()),
        })
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Slot::*;
        write!(
            f,
            "{}",
            match self {
                Shovel => "shovel",
                Weapon => "weapon",
                Head => "head",
                Feet => "feet",
                Body => "body",
                Ring => "ring",
                Spell => "spell",
                Torch => "torch",
                Action => "action",
                Misc => "misc",
                Other => "other",
            }
        )
    }
}

#[derive(Debug)]
struct UI {
    root: Element,
    items: Vec<Item>,
    characters: Vec<Character>,
    current_character: usize,
    current_char_pick: pick_list::State<CharPick>,
    slots: Vec<SlotUI>,
    item_choices: Vec<button::State>,
    menu_location: Option<MenuLocation>,
}

#[derive(Debug)]
struct SlotUI {
    slot: Slot,
    button: button::State,
    items: Vec<button::State>,
}

#[derive(Debug)]
enum MenuLocation {
    Shovel,
    Weapon(Option<WeaponType>),
    Head,
    Feet,
    Body,
    Ring,
    Spell,
    Torch,
    Action,
    Misc,
    Other(Option<OtherItemType>),
}

impl MenuLocation {
    fn to_slot(&self) -> Slot {
        use MenuLocation::*;
        match self {
            Shovel => Slot::Shovel,
            Weapon(_) => Slot::Weapon,
            Head => Slot::Head,
            Feet => Slot::Feet,
            Body => Slot::Body,
            Ring => Slot::Ring,
            Spell => Slot::Spell,
            Torch => Slot::Torch,
            Action => Slot::Action,
            Misc => Slot::Misc,
            Other(_) => Slot::Other,
        }
    }

    fn from_slot(s: &Slot) -> Self {
        use MenuLocation::*;
        match s {
            Slot::Shovel => Shovel,
            Slot::Head => Head,
            Slot::Weapon => Weapon(None),
            Slot::Feet => Feet,
            Slot::Body => Body,
            Slot::Ring => Ring,
            Slot::Spell => Spell,
            Slot::Torch => Torch,
            Slot::Action => Action,
            Slot::Misc => Misc,
            Slot::Other => Other(None),
        }
    }
}

#[derive(Debug)]
enum WeaponType {
    Dagger,
    Broadsword,
    Longsword,
    Whip,
    Spear,
    Rapier,
    Bow,
    Crossbow,
    Other,
}

#[derive(Debug)]
enum OtherItemType {
    Other,
}

#[derive(Debug, Clone)]
enum Message {
    CharPicked(usize),
    SlotPressed(Slot),
    CurseSlot(Slot, bool),
}

#[derive(PartialEq, Eq, Debug, Clone)]
struct CharPick {
    idx: usize,
    name: String,
}

impl fmt::Display for CharPick {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Application for UI {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = NDXData;

    fn new(ndxdata: NDXData) -> (Self, Command<Message>) {
        let NDXData {
            root,
            items,
            characters,
        } = ndxdata;
        (
            Self {
                root,
                items,
                characters,
                current_character: 0,
                current_char_pick: Default::default(),
                slots: Slot::all()
                    .into_iter()
                    .map(|slot| SlotUI {
                        slot,
                        button: Default::default(),
                        items: vec![],
                    })
                    .collect(),
                menu_location: None,
                item_choices: vec![],
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Unawareness".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        eprintln!("got message {:?}", message);

        use Message::*;
        match message {
            CharPicked(i) => {
                self.current_character = i;
                self.menu_location = None;
                self.slots = Slot::all()
                    .into_iter()
                    .map(|slot| SlotUI {
                        slot,
                        button: Default::default(),
                        items: vec![],
                    })
                    .collect();
                Command::none()
            }
            SlotPressed(s) => {
                if self
                    .menu_location
                    .as_ref()
                    .map(MenuLocation::to_slot)
                    .as_ref()
                    == Some(&s)
                {
                    self.menu_location = None;
                } else {
                    self.menu_location = Some(MenuLocation::from_slot(&s));
                }
                Command::none()
            }
            CurseSlot(s, b) => {
                if b {
                    self.characters[self.current_character].curses.insert(s);
                } else {
                    self.characters[self.current_character].curses.remove(&s);
                }
                Command::none()
            }
        }
    }

    fn view(&mut self) -> IcedElement<Message> {
        let char = &self.characters[self.current_character];

        let char_picker = PickList::new(
            &mut self.current_char_pick,
            self.characters
                .iter()
                .enumerate()
                .map(|(idx, c)| CharPick {
                    idx,
                    name: c.name.clone(),
                })
                .collect::<Vec<_>>(),
            Some(CharPick {
                idx: self.current_character,
                name: char.name.clone(),
            }),
            |p| Message::CharPicked(p.idx),
        );

        // let items_by_id: HashMap<String, &Item> = items.iter().map(|i| (i.id.clone(), i)).collect();
        // let items_by_slot: HashMap<Slot, Vec<&Item>> = items.iter().map(|i| (i.slot.parse(), i)).into_group_map();
        //
        let slots = self
            .slots
            .iter_mut()
            .map(
                |SlotUI {
                     slot,
                     button,
                     items,
                 }| {
                    let slot_button = Button::new(button, Text::new(slot.to_string()))
                        .on_press(Message::SlotPressed(slot.clone()));
                    let s2 = slot.clone();
                    let cursed_checkbox = Checkbox::new(char.curses.contains(slot), "cursed", move |b| {
                        Message::CurseSlot(s2.clone(), b)
                    });
                    Column::new().push(slot_button).push(cursed_checkbox).into()
                },
            )
            .collect();

        Row::new()
            .push(char_picker)
            .push(Row::with_children(slots))
            .into()
    }
}

struct NDXData {
    root: Element,
    items: Vec<Item>,
    characters: Vec<Character>,
}

fn load_necrodancer_xml() -> Result<NDXData, MainError> {
    let mut reader = Reader::from_reader(BufReader::new(
        File::open("mods/Unawareness/necrodancer.xml")
            .or_else(|_| File::open("data/necrodancer.xml"))?,
    ));
    let root = Element::from_reader(&mut reader)?;

    let flyaway_re = Regex::new(r"^\|[^\|]*\|([^\|]*)\|$").unwrap();
    let items_e = root
        .get_child("items", NSChoice::None)
        .ok_or_else(|| MainError::BadNecroXML("missing <items> tag".to_string()))?;
    let items: Vec<Item> = items_e
        .children()
        .map(|i| {
            let id = i.name().to_string();
            let flyaway = i.attr("flyaway").unwrap_or(&id[..]);
            let name = flyaway_re
                .captures(flyaway)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())
                .unwrap_or(flyaway)
                .to_string();
            let slot = if let Some(s) = i.attr("slot") {
                s.parse()
                    .map_err(|_| MainError::BadNecroXML(format!("bad item slot: {}", s)))?
            } else {
                Slot::Other
            };
            let image = i.text();
            Ok(Item {
                id,
                name,
                slot,
                image,
            })
        })
        .collect::<Result<_, MainError>>()?;

    let characters_e = root
        .get_child("characters", NSChoice::None)
        .ok_or_else(|| MainError::BadNecroXML("missing <characters> tag".to_string()))?;
    let names: HashMap<&str, &str> = vec![
        ("0", "Cadence"),
        ("1", "Melody"),
        ("2", "Aria"),
        ("3", "Dorian"),
        ("4", "Eli"),
        ("5", "Monk"),
        ("6", "Dove"),
        ("7", "Coda"),
        ("8", "Bolt"),
        ("9", "Bard"),
        ("10", "Nocturna"),
        ("11", "Diamond"),
        ("12", "Mary"),
        ("13", "Tempo"),
        ("14", "Reaper"),
    ]
    .into_iter()
    .collect();
    let characters: Vec<Character> = characters_e
        .children()
        .map(|c| {
            if !c.is("character", NSChoice::None) {
                return Err(MainError::BadNecroXML("bad character tag".to_string()));
            }
            let id = c
                .attr("id")
                .ok_or_else(|| MainError::BadNecroXML("character missing id attr".to_string()))?
                .to_string();
            let name = names.get(&id[..]).unwrap_or(&&id[..]).to_string();
            let c = c
                .get_child("initial_equipment", NSChoice::None)
                .ok_or_else(|| MainError::BadNecroXML("missing <initial_equipment>".to_string()))?;
            let mut items = Vec::new();
            let mut curses = HashSet::new();
            for x in c.children() {
                if x.is("item", NSChoice::None) {
                    items.push(
                        x.attr("type")
                            .ok_or_else(|| {
                                MainError::BadNecroXML("item missing type attr".to_string())
                            })?
                            .to_string(),
                    );
                } else if x.is("cursed", NSChoice::None) {
                    curses.insert(
                        x.attr("slot")
                            .ok_or_else(|| {
                                MainError::BadNecroXML("cursed missing slot attr".to_string())
                            })?
                            .parse()
                            .map_err(|_| MainError::BadNecroXML("bad cursed slot".to_string()))?,
                    );
                } else {
                    return Err(MainError::BadNecroXML("bad initial equipment".to_string()));
                }
            }
            Ok(Character {
                id,
                name,
                items,
                curses,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(NDXData {
        root,
        items,
        characters,
    })
}

fn character_element(chars: &[Character]) -> Element {
    Element::builder("characters")
        .append_all(chars.iter().map(|c| {
            Element::builder("character").attr("id", &c.id[..]).append(
                Element::builder("initial_equipment")
                    .append_all(
                        c.items
                            .iter()
                            .map(|i| Element::builder("item").attr("type", i)),
                    )
                    .append_all(
                        c.curses
                            .iter()
                            .map(|c| Element::builder("cursed").attr("slot", c.to_string())),
                    ),
            )
        }))
        .build()
}

fn main_2() -> Result<(), MainError> {
    let necrodancer_path = dotenv::var("NECRODANCER_PATH")
        .map_err(|e| MainError::DotEnv("NECRODANCER_PATH".to_string(), e))?;
    std::env::set_current_dir(necrodancer_path)?;
    create_dir_all("mods/Unawareness")?;

    let ndxdata = load_necrodancer_xml()?;

    // characters[11].items.push("weapon_cat".to_string());
    // let new_chars = character_element(&characters[..]);
    // root.remove_child("characters", NSChoice::None);
    // root.append_child(new_chars);
    // root.write_to(&mut File::create("mods/Unawareness/necrodancer.xml")?);

    UI::run(Settings::with_flags(ndxdata))?;
    Ok(())
}

fn main() -> Result<(), MainError> {
    main_2().map_err(|e| {
        println!("{}", e);
        e
    })
}