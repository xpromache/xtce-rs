use crate::mdb::*;
use core::fmt;
use std::fmt::{Formatter};

pub struct MdbItemDebug<'a, T> {
    item: &'a T,
    mdb: &'a MissionDatabase,
}

impl std::fmt::Debug for MdbItemDebug<'_, SpaceSystem> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mdb = self.mdb;
        let ss = self.item;

        let ssname = ss.fqn.to_string(mdb.name_db_ref());
        writeln!(f, "SpaceSystem {}", ssname)?;
        writeln!(f, "\t{} Containers: ", ss.containers.len())?;
        for (_, v) in &ss.containers {
            let pt = mdb.get_container(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }

        writeln!(f, "\t{} Parameter Types: ", ss.parameter_types.len())?;
        for (_, v) in &ss.parameter_types {
            let pt = mdb.get_parameter_type(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }
        writeln!(f, "\t{} Parameters: ", ss.parameters.len())?;
        for (_, v) in &ss.parameters {
            let pt = mdb.get_parameter(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }
        Ok(())
    }
}

fn write_debug_units(f: &mut Formatter<'_>, units: &Vec<UnitType>) -> fmt::Result {
    if units.len() > 0 {
        write!(f, ", units: {:?})", units)?;
    }
    Ok(())
}

impl std::fmt::Debug for MdbItemDebug<'_, DataType> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mdb = self.mdb;
        let dtype = self.item;
        write!(f, "DataType(name: {}, {:?}", mdb.name2str(dtype.name()), dtype.type_data)?;
        write!(f, "encoding: {:?}", dtype.encoding)?;
        write!(f, ")")?;
        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, Parameter> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let p = self.item;
        write!(f, "Parameter(name: {}, ", self.mdb.name2str(p.name()))?;
        if let Some(ptype_idx) = p.ptype {
            let ptype = self.mdb.get_parameter_type(ptype_idx);
            write!(f, "type: {}, ", self.mdb.name2str(ptype.name()))?;
        }
        write!(f, "data_source: {:?}", p.data_source)?;
        write!(f, ")")?;

        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, SequenceContainer> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let container = self.item;
        let mdb  = self.mdb;
        write!(f, "SequenceContainer(name: {}", mdb.name2str(container.name()))?;
        if let Some((cidx, mc)) = &container.base_container {
            let bc = mdb.get_container(*cidx);
            write!(f, ", base: {}", mdb.name2str(bc.name()))?;
        }
        writeln!(f, "):")?;
        for entry in container.entries.iter() {
            match &entry.location_in_container {
                Some(lic) =>  write!(f, "\t\t\t|-> {:?} ", lic)?,
                None => write!(f, "\t\t\t|->")?
            }
            match entry.data {
                ContainerEntryData::ParameterRef(pidx) => {
                    let para = mdb.get_parameter(pidx);
                    writeln!(f, "{}", mdb.name2str(para.name()))?;
                },
                ContainerEntryData::ContainerRef(_) => todo!(),
                ContainerEntryData::IndirectParameterRef(_) => todo!(),
                ContainerEntryData::ArrayParameterRef(_) => todo!(),
            }
        }
        

        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, std::collections::HashMap<QualifiedName, Index>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (k, v) in self.item {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", k.to_string(self.mdb.name_db_ref()), v.index())?;
        }

        Ok(())
    }
}

impl std::fmt::Debug for MissionDatabase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mission Database with {} space systems", self.space_systems.len())?;
        for ss in &self.space_systems {
            writeln!(f, "{:?}", MdbItemDebug { item: ss, mdb: self })?;
        }
        Ok(())
    }
}
