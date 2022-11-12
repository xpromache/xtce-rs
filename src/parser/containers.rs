use roxmltree::Node;

use crate::{parser::utils::{read_mandatory_attribute, read_name_description, read_attribute}, mdb::{MissionDatabase, NameReferenceType, NameDescription, SequenceContainer, QualifiedName}};

use super::{ParseContext, XtceError};

pub(super) fn add_container(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<(), XtceError> {
    let node = &ctx.node;
    let abstract_ = read_attribute::<bool>(&ctx.node, "abstract")?.unwrap_or(true);
    let mut ndescr = NameDescription::new(ctx.name);
    read_name_description(&mut ndescr, node);

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "EntryList" => {
                read_entry_list(mdb, ctx.path, &cnode);
            }
            "BaseContainer" => {
                read_entry_list(mdb, ctx.path, &cnode);
            }
            _ => log::warn!("ignoring container unknown property '{}'", cnode.tag_name().name())
        };
    }


    let sc = SequenceContainer {
        ndescr,
        base_container: None,
        abstract_,
        entries: Vec::new()
    };
    mdb.add_seq_container(ctx.path, sc);
    Ok(())
}

fn read_entry_list(mdb: &MissionDatabase, ss: &QualifiedName, node: &Node) {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "ParameterRefEntry" => {},
            "ContainerRefEntry" => {},
            "IndirectParameterRefEntry" => {},
            "ArrayParameterRefEntry" => {},
            _ => log::warn!("ignoring sequence container entry list unknown property '{}'", cnode.tag_name().name())
        }
    }
}