use smallvec::SmallVec;

use crate::{error::MdbError, value::{Value}};

use super::{
    types::{DataType, MemberPath, PathElement, TypeData},
    MissionDatabase, NameDb, NamedItem,
};

///
///   parses a path element from a string like
///
/// name1[idx_1][idx_2][idx_n].name2[jdx_1][jdx_n].name3.name4
///
/// If any of the name is not known in the name db, an InvalidReference error is returned
pub(crate) fn parse_aggregate_member_path(
    name_db: &NameDb,
    path: Vec<&str>,
) -> Result<MemberPath, MdbError> {
    let mut mp = Vec::with_capacity(path.len());

    for p in path {
        if !p.is_empty() {
            mp.push(parse_aggregate_path_element(name_db, p)?);
        }
    }

    Ok(mp)
}

pub(crate) fn parse_aggregate_path_element(
    name_db: &NameDb,
    path: &str,
) -> Result<PathElement, MdbError> {
    let mut it = path.split("[");

    let mut name_str = it.next().map(|x| &x[..x.len()]);
    if let Some("") = name_str {
        name_str = None;
    }
    let mut index = SmallVec::new();

    let name = if let Some(x) = name_str {
        if !name_db.contains(x) {
            return Err(MdbError::InvalidValue(format!(
                "Cannot parse aggregate member path '{}'; '{}' is not known in the name database",
                path, x
            )));
        }
        name_db.get(x)
    } else {
        None
    };

    for v in it {
        if let Some(']') = v.chars().last() {
            let istr = &v[..(v.len() - 1)];
            let idx = istr.parse::<u32>().map_err(|_| {
                MdbError::InvalidValue(format!(
                    "Cannot parse aggregate member path '{}': '{}' is not a positive integer",
                    path, istr
                ))
            })?;
            index.push(idx);
        } else {
            return Err(MdbError::InvalidValue(format!(
                "Cannot parse aggregate member path '{}'; ']' is missing",
                path
            )));
        }
    }

    Ok(PathElement { name, index })
}

/// searches the given type for a member with the given path and returns its data type or None if not found
pub(crate) fn get_member_type<'a>(
    mdb: &'a MissionDatabase,
    dtype: &'a DataType,
    path: &MemberPath,
) -> Option<&'a DataType> {
    let mut rtype = dtype;

    for pe in path {
        if let Some(name) = pe.name {
            if let TypeData::Aggregate(atype) = &rtype.type_data {
                if let Some(m) = atype.members.iter().find(|m| m.name() == name) {
                    rtype = mdb.get_data_type(m.dtype);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        if !pe.index.is_empty() {
            if let TypeData::Array(atype) = &rtype.type_data {
                if atype.dim.len() != pe.index.len() {
                    return None;
                }
                rtype = mdb.get_data_type(atype.dtype);
            } else {
                return None;
            }
        }
    }

    Some(rtype)
}

pub(crate) fn get_member_value<'a>(
    value: &'a Value,
    path: &MemberPath,
) -> Option<&'a Value> {
    let mut val = value;

    for pe in path {
        if let Some(name) = pe.name {
            if let Value::Aggregate(aggrv) = val {
                if let Some(v) = aggrv.0.get(&name) {
                    val = v;
                }  else {
                    return None;
                }
            } else {
                return None;
            }
        }
        if pe.index.len() > 0 {
            todo!()
        }
    }

    Some(val)
}



#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_element() {
        let mdb = &MissionDatabase::new();
        let name_db = &mdb.name_db;
        name_db.get_or_intern("a");
        let r = parse_aggregate_path_element(name_db, "a[2][3]").unwrap();

        assert_eq!("a", mdb.name2str(r.name.unwrap()));
        assert_eq!(SmallVec::from_buf([2, 3]), r.index);

        name_db.get_or_intern("abcd");
        let r = parse_aggregate_path_element(name_db, "abcd[1][2][3][10][1032]").unwrap();
        assert_eq!("abcd", mdb.name2str(r.name.unwrap()));
        assert_eq!(SmallVec::from_buf([1, 2, 3, 10, 1032]), r.index);

        let r = parse_aggregate_path_element(name_db, "unknown_name[1]");
        assert!(r.is_err());

        let r = parse_aggregate_path_element(name_db, "abcd[a1]");
        assert!(r.is_err());

        let r = parse_aggregate_path_element(name_db, "abcd[a1");
        assert!(r.is_err());

        let r = parse_aggregate_path_element(name_db, "abcd a1]");
        assert!(r.is_err());

        println!("r: {:?}", r);
    }

    #[test]
    fn test_parse_path() {
        let mdb = &MissionDatabase::new();
        mdb.name_db.get_or_intern("a");
        mdb.name_db.get_or_intern("b");
        mdb.name_db.get_or_intern("c");
        let r = parse_aggregate_member_path(&mdb.name_db, vec!["a[2]", "b", "c"]).unwrap();
        assert_eq!(3, r.len());
    }
}
