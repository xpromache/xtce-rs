mod containers;
mod encodings;
mod nametree;
mod parameters;
mod types;
mod utils;
mod misc;

use std::fmt;

use roxmltree::{Document, Node, NodeId, TextPos};
use std::error;

use crate::mdb::*;
use types::*;
use utils::*;

//use crate::parser::types::*;

use std::collections::HashMap;
use std::path::Path;

use self::containers::add_container;
use self::nametree::{build_name_tree, NameTree};
use self::parameters::add_parameter;

#[derive(Debug)]
pub struct XtceParseError {
    pub msg: String,
    pub pos: TextPos,
}

#[derive(Debug)]
pub enum XtceError {
    ParseError(XtceParseError),
    DuplicateName(NameIdx, NodeId),
    /// undefined means that the item is not found in the name tree
    UndefinedReference(String, NameReferenceType),
    // unresolved means that the item has been found in the name tree but it is not
    // added to the MDB because either is encountered later in the file or it depends on other item which is not added
    UnresolvedReference(String, NameReferenceType),
    UnresolvedReferences(String),
    InvalidReference(String),
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
    node: Node<'a, 'a>,
    path: &'a QualifiedName,
    name: NameIdx,
    rtype: NameReferenceType,
}
#[derive(Debug)]
pub struct Reference {
    reference: String,
    rtype: NameReferenceType,
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

pub fn parse(mdb: &mut MissionDatabase, path: &Path) -> Result<(), Box<dyn error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root_element = doc.root_element();
    let mut path = QualifiedName::empty();
    let mut name_tree = NameTree {
        name_db: mdb.name_db(),
        systems: HashMap::new(),
    };
    build_name_tree(&mut name_tree, &mut path, &root_element)?;

    build_mdb(mdb, &name_tree, &doc)?;
    //println!("Have {} xtce nodes", ctx.nodes.len());
    // create_details(mdb, &mut ctx, &doc);
    //  read_space_system(mdb, &mut QualifiedName::empty(), &root_element).or_else(|e| Err(e.into()))
    Ok(())
}

/*************** details **************/
fn build_mdb(mdb: &mut MissionDatabase, name_tree: &NameTree, doc: &Document) -> Result<(), XtceError> {
    let mut unresolved: Vec<(ParseContext, Reference)> = vec![];

    for (path, ssn) in &name_tree.systems {
        println!("Creating space system {}", mdb.qn_to_string(path));
        mdb.new_space_system(path.clone()).unwrap();
        //create space system
        for (ntype, m) in ssn {
            for (name, node_id) in m {
                let node = doc.get_node(*node_id).unwrap();
                let ctx = ParseContext {
                    name_tree,
                    path,
                    name: *name,
                    node,
                    rtype: ntype,
                };
                add_item(mdb, &ctx, &mut unresolved)?;
            }
        }
    }
    while !unresolved.is_empty() {
        println!("loooping---------------------------------------- unresolved: {}", unresolved.len());
        let mut unresolved1: Vec<(ParseContext, Reference)> = vec![];

        for (ctx, _) in &unresolved {
            add_item(mdb, ctx, &mut unresolved1)?;
        }
        if unresolved.len() == unresolved1.len() {
            let refs: Vec<String> = unresolved.into_iter().map(|x| x.1.reference).collect();
            return Err(XtceError::UnresolvedReferences(format!(
                "Unresolved references: {}",
                refs.join(", ")
            )));
        }
        unresolved = unresolved1;
    }
    Ok(())
}

fn add_item<'a>(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext<'a>,
    unresolved: &mut Vec<(ParseContext<'a>, Reference)>,
) -> Result<(), XtceError> {
    let r = match ctx.rtype {
        NameReferenceType::ParameterType => add_parameter_type(mdb, ctx),
        NameReferenceType::Parameter => add_parameter(mdb, ctx),
        NameReferenceType::SequenceContainer => add_container(mdb, ctx),
        _ => {
            println!("todo node type {:?}", ctx.rtype);
            Ok(())
        }
    };

    if let Err(err) = r {
        if let XtceError::UnresolvedReference(reference, rtype) = err {
            unresolved.push((*ctx, Reference { reference, rtype }));
        } else {
            return Err(err);
        }
    }
    Ok(())
}
pub(super) fn read_header(_ss: &mut SpaceSystem, _node: &Node) -> Result<(), XtceParseError> {
    Ok(())
}
