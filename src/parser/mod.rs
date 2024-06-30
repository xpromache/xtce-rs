mod containers;
mod encodings;
mod nametree;
mod parameters;
mod types;
mod utils;
mod misc;

use roxmltree::{Document, Node, NodeId, TextPos, Error};

use crate::mdb::*;
use types::*;
use utils::*;

//use crate::parser::types::*;

use std::collections::HashMap;
use std::path::Path;

use self::containers::add_container;
use self::nametree::{build_name_tree, NameTree};
use self::parameters::add_parameter;

// references to _yamcs_ignore are resolved automatically to an unexisting parameter.
const IGNORE_PARAM_NAME: &str = "_yamcs_ignore";
const INVALID_PARAM_IDX: ParameterIdx = ParameterIdx::invalid();

use thiserror::Error;


#[derive(Debug)]
pub struct XtceParseError {
    pub msg: String,
    pub pos: TextPos,
}

#[derive(Error, Debug)]
pub enum XtceError {
    #[error("IO error")]
    Io(std::io::Error),
    #[error("parse error")]
    Parse(XtceParseError),
    #[error("XML parse error")]
    XMLParse(roxmltree::Error),
    #[error("")]  
    DuplicateName(NameIdx, NodeId),
    /// undefined means that the item is not found in the name tree
     #[error("undefined reference")]
    UndefinedReference(String, NameReferenceType),
    // unresolved means that the item has been found in the name tree but it is not
    // added to the MDB because either is encountered later in the file or it depends on other item which is not added
    #[error("unresolved reference")]
    UnresolvedReference(String, NameReferenceType),
    #[error("unresolved reference")]
    UnresolvedReferences(String),
    #[error("invalid reference")]
    InvalidReference(String),
    #[error("invalid value")]
    InvalidValue(String),
    
}

type Result<T> = std::result::Result<T, XtceError>;


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
/*
impl fmt::Display for XtceParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "An Error Occurred, Please Try Again!") // user-facing output
    }
}
*/

impl std::convert::From<XtceParseError> for XtceError {
    fn from(err: XtceParseError) -> Self {
        XtceError::Parse(err)
    }
}

impl std::convert::From<std::io::Error> for XtceError {
    fn from(err: std::io::Error) -> Self {
        XtceError::Io(err)
    }
}

impl std::convert::From<roxmltree::Error> for XtceError {
    fn from(err: roxmltree::Error) -> Self {
        XtceError::XMLParse(err)
    }
}


pub fn parse(mdb: &mut MissionDatabase, path: &Path) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root_element = doc.root_element();
    let mut path = QualifiedName::empty();
    let mut name_tree = NameTree {
        name_db: mdb.name_db(),
        systems: HashMap::new(),
    };
    build_name_tree(&mut name_tree, &mut path, 0, &root_element)?;

    build_mdb(mdb, &name_tree, &vec![doc])?;
    //println!("Have {} xtce nodes", ctx.nodes.len());
    // create_details(mdb, &mut ctx, &doc);
    //  read_space_system(mdb, &mut QualifiedName::empty(), &root_element).or_else(|e| Err(e.into()))
    Ok(())
}

pub fn parse_files(paths: &[&Path]) -> Result<MissionDatabase> {
    // Read all given files
    //
    // TODO: do this in one iter chain instead of two separate ones?
    // ran into borrow checker complications though
    let contents: Result<Vec<String>> = paths
        .iter()
        .map(|&path| std::fs::read_to_string(path).map_err(XtceError::from))
        .collect();
    let contents = contents?;

    let documents: Result<Vec<roxmltree::Document>> = contents
        .iter()
        .map(|content| roxmltree::Document::parse(&content).map_err(XtceError::from))
        .collect();
    let documents = documents?;

    let mut mdb = MissionDatabase::new();
    let mut name_tree = NameTree {
        name_db: mdb.name_db(),
        systems: HashMap::new(),
    };

    for (i, doc) in documents.iter().enumerate() {
        let root_element = doc.root_element();
        let mut path = QualifiedName::empty();
        build_name_tree(&mut name_tree, &mut path, i, &root_element)?;
    }

    build_mdb(&mut mdb, &name_tree, &documents)?;

    Ok(mdb)
}

/*************** details **************/
fn build_mdb(mdb: &mut MissionDatabase, name_tree: &NameTree, doc: &Vec<Document>) -> Result<()> {
    let mut unresolved: Vec<(ParseContext, Reference)> = vec![];

    for (path, ssn) in &name_tree.systems {
        log::debug!("Creating space system {}", mdb.qn_to_string(path));
        mdb.new_space_system(path.clone()).unwrap();
        //create space system
        for (ntype, m) in ssn {
            for (name, (doc_id, node_id)) in m {
                let node = doc[*doc_id].get_node(*node_id).unwrap();
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
) -> Result<()> {
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
pub(super) fn read_header(_ss: &mut SpaceSystem, _node: &Node) -> Result<()> {
    Ok(())
}
