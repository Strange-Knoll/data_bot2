use std::error::Error;
use std::process::Command;
use std::result;

use crossterm::style::Color;
use sqlx::sqlite::{SqlitePoolOptions, SqliteColumn, SqliteRow};
use sqlx::{prelude::*, Pool, Sqlite, Column};
use sqlx::sqlx_macros::*;
//use sqlx::any::*;

use crate::style::{*, self};

#[derive(Clone)]
pub struct DataBase;

impl DataBase{
    pub async fn create_connection(connection:&str) -> Result<Pool<Sqlite>, sqlx::Error>{
        //sqlx::any::install_default_drivers();


        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(connection).await?;

        //run command cargo sqlx prepare
        /* let command_output = Command::new("cargo")
            .args(&["sqlx", "prepare", "--database-url",format!("sqlite://{}", connection).as_str()])
            .output()
            .expect("failed to execute process sqlx prepare");
        println!("command output: {:?}", command_output); */
        Ok(pool)
    }

    //returns a string of tables and columns in the database
    //this string is formated so it can be displayed to the user or given to a bot
    pub async fn get_database_info(db: Pool<Sqlite>) -> Result<String, Box<dyn Error>>{
        let mut result = String::new();
        let query = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&db).await.expect("select table names failed");
        result.push_str("TABLES\n");
        for table in query{
            let table_name = table.get("name");
            result.push_str("\t");
            result.push_str(table_name);
            result.push_str("\n");
            result.push_str("\t\tCOLUMNS\n");
            let query_string = format!("PRAGMA table_info({})", table_name);
            let columns = sqlx::query(&query_string)
                .fetch_all(&db).await.expect("select column names failed");
            for column in columns{
                result.push_str("\t\t\t");
                result.push_str(format!("{} ({})", 
                    column.get::<&str, &str>("name"), 
                    column.get::<&str, &str>("type")
                ).as_str());
                result.push_str("\n");
            }
        }

        Ok(result)
    }

    pub async fn query(db: Pool<Sqlite>, query_str:String, query_type:String) -> Result<Vec<SqliteRow>, Box<dyn Error>> {
        style::println(
            Color::DarkMagenta, 
            Color::Reset, 
            format!("\nquery: {}", query_str).as_str())?;
        //println!("query type: {}", query_type);
        let mut result = Vec::<SqliteRow>::new();
        match query_type.as_str(){
            "fetch" => {
                //println!("fetching data ...");
                result = sqlx::query(&query_str)
                    .fetch_all(&db).await?;
            },
            "execute" => {
                println!("executing query ...");
                sqlx::query(&query_str)
                    .execute(&db).await?;
            },
            _ => {
                println!("Invalid query type");
            },
        }

        Ok(result)
    }

    pub fn pretty_print_data(data:Vec<SqliteRow>) -> String {
        let mut pretty_print_columns:String = String::new();
        let mut pretty_print:String = String::new();
        
        if data.len() > 0{    
            data[0].columns().iter().for_each(|column|{
                pretty_print_columns.push_str(format!("{} | ", column.name()).as_str());
            });
        }
    
        data.iter().for_each(|row|{
    
            row.columns().iter().for_each(|column|{
                //println!("{}", column.type_info());
                match column.type_info().to_string().as_str(){
                    "BOOLEAN" => pretty_print.push_str(format!("{} | ", row.get::<bool, &str>(column.name())).as_str()),
                    "INTEGER" => pretty_print.push_str(format!("{} | ", row.get::<i32, &str>(column.name())).as_str()),
                    "REAL" => pretty_print.push_str(format!("{} | ", row.get::<f64, &str>(column.name())).as_str()),
                    "TEXT" => pretty_print.push_str(format!("{} | ", row.get::<String, &str>(column.name())).as_str()),
                    "BLOB" => pretty_print.push_str(format!("{} | ", "BLOB").as_str()),
                    "NULL" => {
                        if row.try_get::<&[u8], &str>(column.name()).is_ok(){
                            let data = row.get::<&[u8], &str>(column.name());
                            let data_string = String::from_utf8_lossy(data);
                            pretty_print.push_str(format!("{} | ", data_string).as_str());
                        }else if row.try_get::<String, &str>(column.name()).is_ok(){
                            pretty_print.push_str(format!("{} | ", row.get::<String, &str>(column.name())).as_str());
                        }else if row.try_get::<i32, &str>(column.name()).is_ok(){
                            pretty_print.push_str(format!("{} | ", row.get::<i32, &str>(column.name())).as_str());
                        }else if row.try_get::<f64, &str>(column.name()).is_ok(){
                            pretty_print.push_str(format!("{} | ", row.get::<f64, &str>(column.name())).as_str());
                        }else if row.try_get::<bool, &str>(column.name()).is_ok(){
                            pretty_print.push_str(format!("{} | ", row.get::<bool, &str>(column.name())).as_str());
                        }else{
                            pretty_print.push_str(format!("{} | ", "NULL").as_str());
                        }
                    },
                    _ => println!("Unknown type"),
                }
            });
            pretty_print.push_str("\n");
        });
        println!("-----------------------------------");
        println!("{}", pretty_print_columns);
        println!("{}", pretty_print);
        println!("-----------------------------------");
        //join the two strings together
        let pretty_print = format!("{}\n{}", pretty_print_columns, pretty_print);
        pretty_print
    }
}