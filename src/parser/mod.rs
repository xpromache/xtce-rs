mod encodings;
mod types;
mod parameters;
mod utils;

use std::fmt;

use std::fmt::Pointer;
use roxmltree::{TextPos, Node, NodeId, Document};
use std::error;

use crate::mdb::*;
use types::*;
use parameters::*;
use utils::*;

//use crate::parser::types::*;
use crate::parser::encodings::*;

use std::path::Path;
use std::error::Error;
use crate::mdb::ParameterType::Integer;
use std::ptr::read;
use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use std::ops::DerefMut;
use enum_map::EnumMap;
use lasso::ThreadedRodeo;
use std::sync::Arc;


#[derive(Debug)]
pub struct XtceParseError {
    msg: String,
    pos: TextPos,
}

#[derive(Debug)]
enum XtceError {
    ParseError(XtceParseError),
    DuplicateName(NameIdx, NodeId),
    UnresolvedReference(Reference),
    UnresolvedReferences(String),
}

impl fmt::Display for XtceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "An Error Occurred, Please Try Again!") // user-facing output
    }
}

impl std::error::Error for XtceError {}

#[derive(Copy, Clone)]
struct ParseContext<'a> {
    name_tree: &'a NameTree,
    node: Node<'a,'a>,
    path: &'a QualifiedName,
    name: NameIdx,
    rtype: NameReferenceType
}
#[derive(Debug)]
struct Reference {
    reference: String,
    rtype: NameReferenceType
}

struct NameTree  {
    name_db: Arc<ThreadedRodeo::<NameIdx>>,
    systems: HashMap<QualifiedName, EnumMap<NameReferenceType, HashMap<NameIdx, NodeId>>>,
}

impl NameTree {
    fn add_space_system(&mut self, qn_parent: &QualifiedName, name: &str, node: NodeId) -> Result<NameIdx, XtceError> {
        let mut qn = qn_parent.clone();
        let name_idx = self.name_db.get_or_intern(name);
        qn.push(name_idx);

        if self.systems.contains_key(&qn) {
            return Err(XtceError::DuplicateName(name_idx, node));
        }
        self.systems.insert(qn, EnumMap::new());

        Ok(name_idx)
    }

    fn add_node(&mut self, system: &QualifiedName, name: &str, rtype: NameReferenceType, node: NodeId) -> Result<(), XtceError> {
        let ssn = self.systems.get_mut(system).unwrap();
        let name_idx = self.name_db.get_or_intern(name);
        if ssn[rtype].contains_key(&name_idx) {
            return Err(XtceError::DuplicateName(name_idx, node));
        }
        ssn[rtype].insert(name_idx, node);

        Ok(())
    }
    //looks up the reference pointed by reference is defined
    //
    fn resolve_reference(&mut self, reference: &str, ntype: NameReferenceType, system: &QualifiedName) -> Option<(QualifiedName, NameIdx)> {
        Option::None
    }
}

//used by the functions that return either an item (parameter type, container, etc)
// or a reference which is unresolved and causes the item to not be created
enum ItemOrUnresolved<T> {
    Item(T),
    UnresolvedReference(String),
}


impl fmt::Display for XtceParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "An Error Occurred, Please Try Again!") // user-facing output
    }
}
impl std::error::Error for XtceParseError {}
impl std::convert::From<XtceParseError> for XtceError {
    fn from(err: XtceParseError) -> Self {
        XtceError::ParseError(err)
    }
}


pub(crate) fn parse(mdb: &mut MissionDatabase, path: &Path) -> Result<(), Box<dyn error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root_element = doc.root_element();
    let mut path = QualifiedName::empty();
    let mut name_tree = NameTree{name_db: mdb.name_db(), systems: HashMap::new()};
    build_name_tree(&mut name_tree, &mut path, &root_element)?;

    build_mdb(mdb, &name_tree, &doc);
    //println!("Have {} xtce nodes", ctx.nodes.len());
   // create_details(mdb, &mut ctx, &doc);
    //  read_space_system(mdb, &mut QualifiedName::empty(), &root_element).or_else(|e| Err(e.into()))
    Ok(())
}


fn build_name_tree(tree: &mut NameTree, path: &mut QualifiedName, node: &Node) -> Result<(), XtceError> {
    let name_str = node.attribute("name").unwrap();
    let name_idx = tree.add_space_system(&path, name_str, node.id())?;

    path.push(name_idx);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "SpaceSystem" => {
                build_name_tree(tree, path, &cnode)?;
            }
            "TelemetryMetaData" => {
                build_tm_name_tree(tree, path, &cnode)?;
            }
            "CommandMetaData" => {
              //  read_command_meta_data(mdb, ctx, &cnode)?;
            }
            "" => {}
            _ => println!("ignoring '{}'", cnode.tag_name().name())
        };
    }
    path.pop();

    Ok(())
}

fn build_tm_name_tree(tree: &mut NameTree, path: &mut QualifiedName, node: &Node) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "ParameterTypeSet" => {
                for ptnode in cnode.children().filter(|n| n.tag_name().name() != "") {
                    let name = read_mandatory_name(&ptnode)?;
                    tree.add_node(path, name, NameReferenceType::ParameterType, node.id())?;
                }
            }
            "ParameterSet" => {
                for ptnode in cnode.children().filter(|n| !n.tag_name().name().is_empty()) {
                    let name = read_mandatory_name(&ptnode)?;
                    println!("Adding parameter {}", name);
                    tree.add_node(path, name, NameReferenceType::Parameter, node.id())?;
                }
            }
            "ContainerSet" => {
                //read_container_set(mdb, ctx, &cnode)?;
            }
            "AlgorithmSet" => {
                //read_algorithm_set(mdb, ctx, &cnode)?;
            }
            _ => println!("ignoring '{}'", cnode.tag_name().name())
        };
    }
    Ok(())
}



/*************** details **************/
fn build_mdb(mdb: &mut MissionDatabase, name_tree: &NameTree, doc: &Document) -> Result<(), XtceError> {
    println!("Sizeof ctx is {}", std::mem::size_of::<ParseContext>());
    let mut unresolved: Vec<(ParseContext, Reference)> = vec![];

    for (path, ssn) in &name_tree.systems {
            println!("Creating space system {}", mdb.qn_to_string(path));
        //create space system
        for (ntype, m) in ssn {
            for (name, node_id) in m {
                let node = doc.get_node(*node_id).unwrap();
                let ctx = ParseContext {
                    name_tree, path, name: *name, node,
                    rtype: ntype
                };
                add_item(mdb, &ctx, &mut unresolved)?;
            }
        }
    };
    while !unresolved.is_empty() {
        let mut unresolved1: Vec<(ParseContext, Reference)> = vec![];

        for (ctx,_) in &unresolved {
            add_item(mdb, ctx, &mut unresolved1);
        }
        if unresolved.len() == unresolved1.len() {
            return Err(XtceError::UnresolvedReferences("TODO".to_string()));
        }
        unresolved = unresolved1;
    }
    Ok(())

}

fn add_item<'a>(mdb: &mut MissionDatabase, ctx: &ParseContext<'a>, unresolved: &mut Vec<(ParseContext<'a>, Reference)>) -> Result<(), XtceError> {
    let r= match ctx.rtype {
        NameReferenceType::ParameterType => {
            add_parameter_type(mdb, ctx)
        },/*
                    NameReferenceType::Parameter => {
                        println!("bubu parameter :{:?}", ntype);
                    }*/
        _ => {
            println!("todo node type {:?}", ctx.rtype);
            Ok(())
        }
    };


    if let Err(err)  = r {
        if let XtceError::UnresolvedReference(reference) = err {
            unresolved.push((*ctx, reference));
        } else {
            return  Err(err);
        }
    }
    Ok(())
}
pub(super) fn read_header(ss: &mut SpaceSystem, node: &Node) -> Result<(), XtceParseError> {
    Ok(())
}
