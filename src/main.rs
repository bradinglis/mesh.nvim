
use markdown_db::*;

fn main() -> Result<()> {

    bin_do("/home/inglisb/.cache/markdown_db.duckdb".to_string(),"/home/inglisb/testing/zettel/".to_string(), |db| db.process_files())?;
    
    // let x = bin_do("/home/inglisb/.cache/markdown_db.duckdb".to_string(),"/home/inglisb/testing/zettel/".to_string(), |db| db.get_notes().map(Some))?;

    // let x = bin_do("/home/inglisb/.cache/markdown_db.duckdb".to_string(),"/home/inglisb/testing/zettel/".to_string(), |db| db.get_tag_list())?;
    let x = bin_do("/home/inglisb/.cache/markdown_db.duckdb".to_string(),"/home/inglisb/testing/zettel/".to_string(), |db| db.get_tag_list())?;

    println!("{:#?}", x);

    Ok(())
}



