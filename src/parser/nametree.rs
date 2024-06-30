use crate::mdb::{NameDb, NameIdx, NameReferenceType, QualifiedName, types::MemberPath, utils::parse_aggregate_member_path};
use enum_map::EnumMap;
use std::collections::HashMap;

use super::{utils::read_mandatory_name, XtceError};

pub(crate) struct NameTree {
    pub name_db: NameDb,
    pub systems:
        HashMap<QualifiedName, EnumMap<NameReferenceType, HashMap<NameIdx, (usize, roxmltree::NodeId)>>>,
}

impl NameTree {
    fn add_system(
        &mut self,
        name: &str,
        node: roxmltree::NodeId,
    ) -> Result<QualifiedName, XtceError> {
        let mut qn = self.qn(name);
        if qn.is_root() {
            return Ok(qn);
        }

        if self.systems.contains_key(&qn) {
            return Err(XtceError::DuplicateName(qn.pop().unwrap(), node));
        }
        self.systems.insert(qn.clone(), EnumMap::default());

        Ok(qn)
    }

    fn add_sub_system(
        &mut self,
        qn_parent: &QualifiedName,
        name: &str,
        node: roxmltree::NodeId,
    ) -> Result<NameIdx, XtceError> {
        let mut qn = qn_parent.clone();
        let name_idx = self.name_db.get_or_intern(name);
        qn.push(name_idx);

        if self.systems.contains_key(&qn) {
            return Err(XtceError::DuplicateName(name_idx, node));
        }
        self.systems.insert(qn, EnumMap::default());

        Ok(name_idx)
    }

    fn add_node(
        &mut self,
        system: &QualifiedName,
        name: &str,
        rtype: NameReferenceType,
        doc_id: usize,
        node: roxmltree::NodeId,
    ) -> Result<(), XtceError> {
        let ssn = self.systems.get_mut(system).unwrap();
        let name_idx = self.name_db.get_or_intern(name);
        if ssn[rtype].contains_key(&name_idx) {
            return Err(XtceError::DuplicateName(name_idx, node));
        }
        ssn[rtype].insert(name_idx, (doc_id, node));

        Ok(())
    }

    /// looks up the reference in the name tree returning a tuple
    /// (space system name, item name, member path)
    ///
    /// the reference may be relative "a/b" or absolute "/a/b/c". It can contain ".." or "./"
    ///
    /// The member path may be returned if the item sought is a parameter.
    /// Note that the path is not checked for existance, because that requires looking up the parameter type.
    ///
    /// If rtype is anything else than Parameter, the member path will always be None
    ///
    pub(crate) fn resolve_ref(
        &self,
        reference: &str,
        relative_to: &QualifiedName,
        rtype: NameReferenceType,
    ) -> Option<(&QualifiedName, NameIdx, Option<MemberPath>)> {
        if reference.starts_with("/") {
            return self.find_ref(reference, &QualifiedName::empty(), rtype);
        } else if reference.starts_with("./") || reference.starts_with("..") {
            return self.find_ref(reference, relative_to, rtype);
        } else {
            // relative reference, we try to match it on any path up until the root
            let mut start_ss = relative_to.clone();

            loop {
                let rr = self.find_ref(reference, &start_ss, rtype);
                if rr.is_some() {
                    return rr;
                }
                if start_ss.is_root() {
                    return None;
                }
                start_ss.pop();
            }
        }
    }

    // find relative space system reference
    fn find_ref(
        &self,
        reference: &str,
        relative_to: &QualifiedName,
        rtype: NameReferenceType,
    ) -> Option<(&QualifiedName, NameIdx, Option<MemberPath>)> {
        let mut ss = relative_to.clone();
        let mut it = reference.split('/').peekable();

        while let Some(p) = it.next() {
            if p == "." || p == "" {
                continue;
            }
            if p == ".." {
                if !ss.is_root() {
                    ss.pop();
                } //else we consider that /.. = / (i.e. the root is its own parent)
                continue;
            }

            if it.peek().is_none() {
                //reached the end, check we have an item of the correct type
                //last component could be a path into an aggregate parameter

                let mut member_path = None;
                let mut pname = p;
                if rtype == NameReferenceType::Parameter {
                    if let Some(n) = p.find('.') {
                        let (a, b) = p.split_at(n);
                        member_path = Some(parse_aggregate_member_path(&self.name_db, b[1..].split('.').collect()).ok()?);
                        pname = a;
                    }
                }

                let pidx = self.name_db.get(pname)?;

                return self.systems.get_key_value(&ss).and_then(|(k, v)| {
                    if v[rtype].contains_key(&pidx) {
                        Some((k, pidx, member_path))
                    } else {
                        None
                    }
                });
            }

            let pidx = self.name_db.get(p)?;
            ss.push(pidx);

            if !self.systems.contains_key(&ss) {
                if rtype != NameReferenceType::Parameter {
                    return None;
                } else {
                    //even if we did not reach the end, if we find a parameter with the given name,
                    // we assume the rest is part of a possible aggregate path
                    ss.pop();

                    return self.systems.get_key_value(&ss).and_then(|(k, v)| {
                        if v[rtype].contains_key(&pidx) {
                            let member_path = parse_aggregate_member_path(&self.name_db, it.collect::<Vec<&str>>()).ok()?;
                            Some((k, pidx, Some(member_path)))
                        } else {
                            None
                        }
                    });
                }
            }
        }

        None
    }

   pub fn qn(&self, qnstr: &str) -> QualifiedName {
        let mut r = QualifiedName::empty();
        for p in qnstr.split("/") {
            if !p.is_empty() {
                r.push(self.name_db.get_or_intern(p));
            }
        }
        r
    }
}

pub(crate) fn build_name_tree(
    tree: &mut NameTree,
    path: &mut QualifiedName,
    doc_id: usize,
    node: &roxmltree::Node,
) -> Result<(), XtceError> {
    let name_str = read_mandatory_name(node)?;
    let name_idx = tree.add_sub_system(&path, name_str, node.id())?;

    path.push(name_idx);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "SpaceSystem" => {
                build_name_tree(tree, path, doc_id, &cnode)?;
            }
            "TelemetryMetaData" => {
                build_tm_name_tree(tree, path, doc_id, &cnode)?;
            }
            "CommandMetaData" => {
                //  read_command_meta_data(mdb, ctx, &cnode)?;
            }
            "" => {}
            _ => log::warn!("ignoring global property '{}'", cnode.tag_name().name()),
        };
    }
    path.pop();

    Ok(())
}

fn build_tm_name_tree(
    tree: &mut NameTree,
    path: &mut QualifiedName,
    doc_id: usize,
    node: &roxmltree::Node,
) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "ParameterTypeSet" => {
                for ptnode in cnode.children().filter(|n| n.tag_name().name() != "") {
                    let name = read_mandatory_name(&ptnode)?;
                    tree.add_node(path, name, NameReferenceType::ParameterType, doc_id, ptnode.id())?;
                }
            }
            "ParameterSet" => {
                for ptnode in cnode.children().filter(|n| !n.tag_name().name().is_empty()) {
                    let name = read_mandatory_name(&ptnode)?;
                    tree.add_node(path, name, NameReferenceType::Parameter, doc_id, ptnode.id())?;
                }
            }
            "ContainerSet" => {
                for ptnode in cnode.children().filter(|n| n.tag_name().name() != "") {
                    let name = read_mandatory_name(&ptnode)?;
                    tree.add_node(path, name, NameReferenceType::SequenceContainer, doc_id, ptnode.id())?;
                }
            }
            "AlgorithmSet" => {
                //read_algorithm_set(mdb, ctx, &cnode)?;
            }
            "" => {}
            _ => log::warn!("ignoring '{}'", cnode.tag_name().name()),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use lasso::ThreadedRodeo;
    use roxmltree::NodeId;

    use crate::{
        mdb::{NameIdx, NameReferenceType, QualifiedName},
        parser::nametree::NameTree,
    };

    #[test]
    fn test_find_sysref() {
        let mut ntree = NameTree {
            name_db: Arc::new(ThreadedRodeo::<NameIdx>::new()),
            systems: HashMap::new(),
        };

        let node_id = NodeId::new(0);
        let root = QualifiedName::empty();
        let qn_a = ntree.add_system("/a", node_id).unwrap();
        println!("qn_a: {:?} {}", qn_a, qn_a.to_string(&ntree.name_db));

        let ptype = NameReferenceType::Parameter;

        let qn_b = ntree.add_system("/b", node_id).unwrap();
        let qn_ab = ntree.add_system("/a/b", node_id).unwrap();
        let qn_abc = ntree.add_system("/a/b/c", node_id).unwrap();
        let _qn_bd = ntree.add_system("/b/d", node_id).unwrap();

        ntree.add_node(&qn_ab, "para1", ptype, 0, node_id).unwrap();
        ntree.add_node(&qn_abc, "para2", ptype, 0, node_id).unwrap();
        ntree.add_node(&qn_b, "para3", ptype, 0, node_id).unwrap();

        let x = ntree.find_ref("/x", &root, ptype);
        assert!(x.is_none());

        let x = ntree.find_ref("/a/x", &root, ptype);
        assert!(x.is_none());

        let (x, _, _) = ntree.find_ref("/a/b/para1", &root, ptype).unwrap();
        assert_eq!(x, &qn_ab);

        let (x, _, _) = ntree.find_ref("c/para2", &qn_ab, ptype).unwrap();
        assert_eq!(x, &qn_abc);

        let (x, _, _) = ntree.resolve_ref("../b/para1", &qn_ab, ptype).unwrap();
        assert_eq!(x, &qn_ab);

        let (x, _, _) = ntree.resolve_ref("b/c/para2", &qn_a, ptype).unwrap();
        assert_eq!(x, &qn_abc);

        let x = ntree.resolve_ref("b/c/para1", &qn_a, ptype);
        assert!(x.is_none());

        let (x, _, _) = ntree.resolve_ref("a/b/para1", &qn_abc, ptype).unwrap();
        assert_eq!(x, &qn_ab);

        let (x, _, pn) = ntree.resolve_ref("b/para3/a/b/c", &qn_abc, ptype).unwrap();
        assert_eq!(x, &qn_b);
        assert_eq!(3, pn.unwrap().len());

        let (x, _, pn) = ntree.resolve_ref("b/para3.a.b.c", &qn_abc, ptype).unwrap();
        assert_eq!(x, &qn_b);
        assert_eq!(3, pn.unwrap().len());

        let (x, _, pn) = ntree.resolve_ref("/b/para3", &qn_abc, ptype).unwrap();
        assert_eq!(x, &qn_b);
        assert!(pn.is_none());
    }
}
