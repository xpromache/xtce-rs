use smallvec::SmallVec;

use crate::error::MdbError;

use super::{
    types::{MemberPath, PathElement},
    MissionDatabase, NameDb,
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

    Ok(PathElement{ name, index})
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
        assert_eq!(SmallVec::from_buf([2,3]), r.index);

        name_db.get_or_intern("abcd");
        let r = parse_aggregate_path_element(name_db, "abcd[1][2][3][10][1032]").unwrap();
        assert_eq!("abcd", mdb.name2str(r.name.unwrap()));
        assert_eq!(SmallVec::from_buf([1,2,3,10,1032]), r.index);
        
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
        let r = parse_aggregate_member_path(&mdb.name_db, vec!["a[2]","b","c"]).unwrap();
        assert_eq!(3, r.len());
    }
}
