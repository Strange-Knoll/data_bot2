use std::{env, path::PathBuf, process::Command, };

use async_openai::{Client, types::{CreateThreadRequestArgs, CreateAssistantRequestArgs, AssistantTools, AssistantToolsFunction, ChatCompletionFunctions, CreateMessageRequestArgs, CreateRunRequestArgs, RunStatus, MessageContent, StepDetails, RunStepDetailsToolCalls, SubmitToolOutputsRunRequest, ToolsOutputs}, config::OpenAIConfig};
use crossterm::style::Color;

use serde_json::{self, Value};

mod style;
//use crate::style::*;
mod ledit;
//use crate::ledit::*;
mod sql_ops;
use sql_ops::DataBase;
use sqlx::{Pool, Sqlite, sqlite::SqliteRow};

//fix the thing where it hard quits if you dont have a key


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //clear the screen
    print!("{}[2J", 27 as char);

    // print introduction to the user
    style::print(Color::Reset, Color::Reset, "Hello I am ")?;
    style::print(Color::Magenta, Color::Reset, "Data")?;
    style::print(Color::Reset, Color::Reset, ". I am an assistant that can help you explore sqlite databases.\n")?;
    //print data message
    style::print(Color::Reset, Color::Reset, "Type ")?;
    style::print(Color::Magenta, Color::Reset, "data")?;
    style::print(Color::Reset, Color::Reset, " followed by your query to ask me questions about the database.\n")?;
    //print help message
    style::print(Color::Reset, Color::Reset, "Type ")?;
    style::print(Color::Green, Color::Reset, "help")?;
    style::print(Color::Reset, Color::Reset, " to see a list of commands.\n\n")?;
    //print connect message
    style::print(Color::Reset, Color::Reset, "Type ")?;
    style::print(Color::Blue, Color::Reset, "connect")?;
    style::print(Color::Reset, Color::Reset, " followed by the path to a database to connect to it.\n")?;
    //print disconnect message
    style::print(Color::Reset, Color::Reset, "Type ")?;
    style::print(Color::Blue, Color::Reset, "disconnect")?;
    style::print(Color::Reset, Color::Reset, " to disconnect from a database.\n\n")?;
    //print exit message
    style::print(Color::Reset, Color::Reset, "Type ")?;
    style::print(Color::Red, Color::Reset, "exit")?;
    style::print(Color::Reset, Color::Reset, " to exit the program.\n\n")?;
    let key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            style::println(Color::Red, Color::Reset, "Error: OPENAI_API_KEY not found")?;
            style::print(Color::Reset, Color::Reset, "please set your key using ")?;
            style::print(Color::Green, Color::Reset, "OPENAI_API_KEY")?;
            style::println(Color::Reset, Color::Reset, " environment variable")?;
            //exit the program
            std::process::exit(1);
        }
    };
    let config = OpenAIConfig::default().with_api_key(key);
    let client = Client::with_config(config);
    
    let thread_request = CreateThreadRequestArgs::default()
        .build()?;
    let thread = client.threads().create(thread_request).await?;
    let assistant_request = CreateAssistantRequestArgs::default()
        .name("Data")
        .model("gpt-3.5-turbo-16k")
        .description("An sql assistant that can help explore sqlite databases.")
        .instructions("
FOLLOW THESE INSTUCTIONS PRECISELY:
you are an AI designed to help people explore sqlite databases.
    you are an expert at sqlite.
            
    you will be given a question from the user,
    you can query the database,
    you can respond to questions from the user which may not require a database query
            
    This assistant can connect to a database, list the tables and columns in the database, and execute queries on the database. 
    DO NOT summarize or display any data yourself after a database request has been made,
        ONLY respond with \"Database Queried\".
    DO NOT create a new message after a function call
            
    you have complete access to the database, and can answer any query about it.
    you have the ability to remember infomration from previous queries
    you have the ability to recall data you've seen before without querying the database

    IF the user tells you not to query the database:
        you are still able to provid information about the database
        use your ability to recall information to attempt to answer the question

    IF the database queries successfully:
        ONLY respond with \"Database Queried\"
            
    IF the user enters a query whos parameters may be invaild given the database:
        DO NOT run the query
        DO explain to the user why the query might be invalid
            
    IF you recieve an empty result from a database query:
        the query is invalid
        DO NOT attempt any corrections
        DO explain to the user why the query might be invalid")
        .tools(vec![
            AssistantTools::Function(AssistantToolsFunction{
                r#type: "function".to_string(),
                function: ChatCompletionFunctions{
                    name:"db_query".to_string(),
                    description:Some("generate a sqlite query to retrieve data requested by the user, 
                        this data will be printed to the user, you do not need to summarize it.".to_string()),
                    parameters:serde_json::from_str::<Value>("{
                        \"type\": \"object\", 
                        \"properties\": {
                            \"query\": {
                                \"type\": \"string\",
                                \"description\": \"the query to be executed\"
                            }
                        },
                        
                        \"required\": [\"query\"]
                    }").unwrap(),
                }
            }),
            //hello world
            AssistantTools::Function(AssistantToolsFunction{
                r#type: "function".to_string(),
                function: ChatCompletionFunctions{
                    name:"hello_world".to_string(),
                    description:Some("print hello world".to_string()),
                    parameters:serde_json::from_str::<Value>("{
                        \"type\": \"object\", 
                        \"properties\": {
                            \"name\": {
                                \"type\": \"string\",
                                \"description\": \"the name of the person to say hello to\"
                            }
                        },
                        
                        \"required\": [\"name\"]
                    }").unwrap(),
                }
            }),
            //connect to database
            AssistantTools::Function(AssistantToolsFunction{
                r#type: "function".to_string(),
                function: ChatCompletionFunctions{
                    name:"connect_to_database".to_string(),
                    description:Some("connect to a sqlite database".to_string()),
                    parameters:serde_json::from_str::<Value>("{
                        \"type\": \"object\", 
                        \"properties\": {
                            \"database\": {
                                \"type\": \"string\",
                                \"description\": \"the path to the database to connect to\"
                            }
                        },
                        
                        \"required\": [\"database\"]
                    }").unwrap(),
                }
            }),
        ])
        .build()?;
    let assistant = client.assistants().create(assistant_request).await?;
    let mut data_base = Vec::<Pool<Sqlite>>::new();
    
    let mut working_dir:PathBuf = env::current_dir()?;
    let mut data_base_path = String::new();
    let user_name = whoami::username();
    let os = whoami::platform();

    //message to the bot
    let message = CreateMessageRequestArgs::default()
        .role("user")
        .content(format!("Hello, my name is {} and I am using {}.", user_name, os).as_str())
        .build()?;
    let _message = client.threads().messages(&thread.id).create(message).await?;
    //message to the bot
    let message = CreateMessageRequestArgs::default()
        .role("user")
        .content("
FOLLOW THESE INSTUCTIONS:
once i have queried a database, 
    DO NOT post a message after you have called the query function,
    DO NOT attempt to summarize or display the data queried,
    ONLY run a function once per query,
    IF i enter a query whos perameters are not met in the database:
        DO NOT run a query,
        DO attempt to explain to me why the query may be invalid
    IF the database is queried successfully:
        ONLY reply with \"query successful\"")
        .build()?;
    let _message = client.threads().messages(&thread.id).create(message).await?;

    // --------
    // MAIN LOOP
    // --------
    loop {
        
        //print the prompt
        let working_path = if data_base.len() > 0 {
            data_base_path.clone()
        }
        else{
            let wd = working_dir.to_str().unwrap().to_string();
            wd
        };
        let input = ledit::pretty_prompt(&user_name, " @ ",
            &working_path, " -> ");

        let input = input.trim();

        let words = input.split(' ').collect::<Vec<&str>>();

        match words[0] {
            "exit" => {
                client.assistants().delete(&assistant.id).await?;
                client.threads().delete(&thread.id).await?;
                break
            },
            "clear" => {
                print!("{}[2J", 27 as char);
                continue;
            },
            "cd" => {
                if words.len() == 1 {
                    match env::set_current_dir(format!("/home/{}", user_name)){
                        Ok(_) => {},
                        Err(_) => {
                            style::println(Color::Red, Color::Reset, "Error: could not change directory")?;
                            continue;
                        }
                    
                    };
                    working_dir = match env::current_dir(){
                        Ok(working_dir) => working_dir,
                        Err(_) => {
                            style::println(Color::Red, Color::Reset, "Error: could not get current directory")?;
                            continue;
                        }
                    };
                    
                    continue;
                }
                else{
                    match env::set_current_dir(words[1]){
                        Ok(_) => {},
                        Err(_) => {
                            style::println(Color::Red, Color::Reset, "Error: could not change directory")?;
                            continue;
                        }
                    
                    
                    };
                    working_dir = match env::current_dir(){
                        Ok(working_dir) => working_dir,
                        Err(_) => {
                            style::println(Color::Red, Color::Reset, "Error: could not get current directory")?;
                            continue;
                        }
                    
                    };
                    continue;
                }
            },
            //list files in current directory using ls command
            "ls" => {
                let args = Vec::new();
                let args = if words.len() == 1 {
                    args
                } else {
                    words[1..].iter().map(|s| s.to_string()).collect()
                };

                let _entries = match Command::new("ls")
                    .args(&args)
                    .status(){
                        Ok(_) => {},
                        Err(_) => {
                            style::println(Color::Red, Color::Reset, "Error: command not found")?;
                            continue;
                        }
                    };
                    
                //style::println(Color::Blue, Color::Reset, &entries)?;
            },
            //create a new database connection
            "connect" =>{
                //style::println(Color::Green, Color::Reset, "connected to db")?;
                let connection = match DataBase::create_connection(words[1]).await {
                    Ok(connection) => connection,
                    Err(_) => {
                        /* style::println(Color::Red, Color::Reset, 
                            format!("Error: could not connect to database: {}", e).as_str()
                        )? ;*/
                        style::print(Color::Red, Color::Reset, "Error: ")?;
                        style::println(Color::Reset, Color::Reset, "could not read database")?;
                        style::println(Color::DarkGrey, Color::Reset, "Have sure you've entered the correct path?")?;
                        continue;
                    }
                };
                data_base.push(connection);
                data_base_path = words[1].to_string();
                
                
                let db_details = match DataBase::get_database_info(data_base[data_base.len()-1].clone()).await {
                    Ok(db_details) => db_details,
                    Err(e) => {
                        style::println(Color::Red, Color::Reset, "Error: could not get database info")?;
                        format!("Error reading database info {}", e)
                    }
                };
                style::println(Color::Blue, Color::Reset, &db_details)?;
                let message = CreateMessageRequestArgs::default()
                    .role("user")
                    .content(format!("connected to Database >>> database info: \n {}", db_details.as_str()).as_str())
                    .build()?;
                let _message = client.threads().messages(&thread.id).create(message).await?;
                style::print(Color::Green, Color::Reset, "connected")?;
                style::print(Color::Reset, Color::Reset, " to ")?;
                style::println(Color::Blue, Color::Reset, &data_base_path)?;
            }
            //disconnect
            "disconnect" => {
                data_base.pop();
                style::print(Color::Green, Color::Reset, "disconnected")?;
                style::print(Color::Reset, Color::Reset, " from ")?;
                style::println(Color::Blue, Color::Reset, &data_base_path)?;
                data_base_path = String::new();
            },
            //help message
            "help" => {
                //print the help message
                style::print(Color::Blue, Color::Reset, "\nCommands:\n\n")?;
                //data
                style::print(Color::Magenta, Color::Reset, "\tdata:\t\t")?;
                style::println(Color::Reset, Color::Reset, "ask data about the database")?;
                //connect
                style::print(Color::Red, Color::Reset, "\tconnect:\t")?;
                style::println(Color::Reset, Color::Reset, "connect to a database")?;
                //disconnect
                style::print(Color::Yellow, Color::Reset, "\tdisconnect:\t")?;
                style::println(Color::Reset, Color::Reset, "disconnect from a database")?;
                style::print(Color::Reset, Color::Reset, "\n")?;
                //help
                style::print(Color::Green, Color::Reset, "\thelp:\t\t")?;
                style::println(Color::Reset, Color::Reset, "display this message")?;
                //exit
                style::print(Color::Cyan, Color::Reset, "\texit:\t\t")?;
                style::println(Color::Reset, Color::Reset, "exit the program")?;
                //clear
                style::print(Color::Blue, Color::Reset, "\tclear:\t\t")?;
                style::println(Color::Reset, Color::Reset, "clear the terminal")?;
                //cd
                style::print(Color::Magenta, Color::Reset, "\tcd:\t\t")?;
                style::println(Color::Reset, Color::Reset, "change directory")?;
                //ls
                style::print(Color::Red, Color::Reset, "\tls:\t\t")?;
                style::println(Color::Reset, Color::Reset, "list files in current directory")?;
                style::print(Color::Reset, Color::Reset, "\n")?;

            },
            "data" => {
                //check if we have a connection to a database
                if data_base.len() == 0 {
                    style::print(Color::Red, Color::Reset, "Error: ")?;
                    style::println(Color::Reset, Color::Reset, "no database connected")?;
                    continue;
                }
                //println!("data");
                style::print(Color::Magenta, Color::Reset, "Data")?;
                style::print(Color::Reset, Color::Reset, " @ ")?;
                style::print(Color::Blue, Color::Reset, &data_base_path)?;
                style::print(Color::Reset, Color::Reset, " -> ")?;
                style::print(Color::Magenta, Color::Reset, "=")?;
                let user_message = CreateMessageRequestArgs::default()
                    .role("user")
                    .content(words[1..].join(" "))
                    .build()?;
                let _message = client
                    .threads()
                    .messages(&thread.id)
                    .create(user_message)
                    .await?;

                let run_request = CreateRunRequestArgs::default()
                    .assistant_id(&assistant.id)
                    .build()?;
                let run = client.threads().runs(&thread.id).create(run_request).await?;

                let mut running = true;
                while running == true {
                //loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let retrieve_run = client.threads().runs(&thread.id).retrieve(&run.id).await?;
                    match retrieve_run.status{
                        RunStatus::InProgress => {
                            style::print(Color::Green, Color::Reset, "=")?;
                        },
                        RunStatus::Completed => {
                            style::println(Color::Green, Color::Reset, "+")?;
                            
                            running = false;

                            //retrieve last message
                            let last_message = client
                                .threads()
                                .messages(&thread.id)
                                .list(&[("limit", "1")])
                                .await?;
                            let last_message_id = &last_message.data[0].id;
                            let last_message = client
                                .threads()
                                .messages(&thread.id)
                                .retrieve(&last_message_id)
                                .await?;
                            match last_message.content[0] {
                                MessageContent::Text(ref text) => {
                                    style::println(Color::Magenta, Color::Reset, &text.text.value)?;
                                },
                                _ => {
                                    style::println(Color::Red, Color::Reset, "Error: last message was not text")?;
                                }
                            }
                            
                        },
                        RunStatus::RequiresAction => {
                            //println!("requires action");
                            style::print(Color::Yellow, Color::Reset, "=")?;
                            let step = client
                                .threads()
                                .runs(&thread.id)
                                .steps(&run.id)
                                .list(&[("limit", "1")])
                                .await?;
                            let step_id = &step.data[0].id;
                            let step = client
                                .threads()
                                .runs(&thread.id)
                                .steps(&run.id)
                                .retrieve(&step_id)
                                .await?;
                            match step.step_details {
                            
                                StepDetails::MessageCreation(message) => {
                                    //println!("message creation");
                                    style::print(Color::Magenta, Color::Reset, "=")?;
                                    let _message = client
                                        .threads()
                                        .messages(&thread.id)
                                        .retrieve(&message.message_creation.message_id)
                                        .await?;
                                    /* for message in message.content {
                                        match message {
                                            MessageContent::Text(text) => {
                                                style::println(Color::Red, Color::Reset, &text.text.value)?;
                                            },
                                            _ => {
                                                style::println(Color::Red, Color::Reset, "Error: message was not text")?;
                                            }
                                        }
                                    } */
                                },
                                StepDetails::ToolCalls(tool_calls) => {
                                    //println!("tool calls");
                                    style::print(Color::Red, Color::Reset, "=")?;
                                    for tool in tool_calls.tool_calls {
                                        match tool {
                                            RunStepDetailsToolCalls::Function(func) => {
                                                //println!("function {}", func.function.name.as_str());
                                                style::print(Color::Magenta, Color::Reset, "=")?;
                                                match func.function.name.as_str(){
                                                    "db_query" => {
                                                        //println!("db query");
                                                        style::print(Color::Blue, Color::Reset, "=")?;
                                                        let query:serde_json::Value = serde_json::from_str(&func.function.arguments.as_str())?;
                                                        let query = match query["query"].as_str() {
                                                            Some(query) => query,
                                                            None => {
                                                                style::println(Color::Red, Color::Reset, "Error: query not found")?;
                                                                continue;
                                                            }
                                                        };
                                                        //style::println(Color::Red, Color::Reset, &query)?;

                                                        let query_response = match DataBase::query(
                                                            data_base[0].clone(), 
                                                            query.to_string(), 
                                                            "fetch".to_string())
                                                            .await{
                                                                Ok(query_response) => query_response,
                                                                Err(e) => {
                                                                    style::println(Color::Red, Color::Reset, "Error: query failed")?;
                                                                    style::println(Color::Red, Color::Reset, format!("{}", e).as_str())?;
                                                                    Vec::<SqliteRow>::new()
                                                                }
                                                            };
                                                            

                                                        let _ = client
                                                        .threads()
                                                        .runs(&thread.id)
                                                        .submit_tool_outputs(&run.id, 
                                                            SubmitToolOutputsRunRequest{
                                                                tool_outputs: vec![
                                                                    ToolsOutputs{
                                                                        tool_call_id: Some(func.id),
                                                                        output: Some(DataBase::pretty_print_data(query_response))
                                                                    }
                                                                ]
                                                            }
                                                        ).await?;
                                                        
                                                    }
                                                    "hello_world" => {
                                                        println!("hello world");
                                                        
                                                        let _ = client
                                                        .threads()
                                                        .runs(&thread.id)
                                                        .submit_tool_outputs(&run.id, 
                                                            SubmitToolOutputsRunRequest{
                                                                tool_outputs: vec![
                                                                    ToolsOutputs{
                                                                        tool_call_id: Some(func.id),
                                                                        output: Some("function executed correctly".to_owned())
                                                                    }
                                                                ]
                                                            }
                                                        ).await?;
                                                        
                                                    }
                                                    _=>{style::println(Color::Red, Color::Reset, "Error: assistant function not found")?;}
                                                }
                                            }
                                            _=>{}
                                        }
                                    }
                                },
                            }
                            //break;
                        },
                        
                        RunStatus::Failed => {
                            style::print(Color::Red, Color::Reset, "Error: ")?;
                            style::println(Color::Reset, Color::Reset, "run failed")?;
                            running = false;
                        },
                        RunStatus::Cancelling => {
                            style::print(Color::Red, Color::Reset, "Error: ")?;
                            style::println(Color::Reset, Color::Reset, "run cancelled")?;
                            running = false;
                        },
                        RunStatus::Cancelled => {
                            style::print(Color::Red, Color::Reset, "Error: ")?;
                            style::println(Color::Reset, Color::Reset, "run cancelled")?;
                            running = false;
                        },
                        RunStatus::Expired => {
                            style::print(Color::Red, Color::Reset, "Error: ")?;
                            style::println(Color::Reset, Color::Reset, "run expired")?;
                            running = false;
                        },
                        RunStatus::Queued => {
                            style::print(Color::Green, Color::Reset, "=")?;
                        
                            running = false;
                        },
                    };
                }
            },
            _ => {
                style::println(Color::Red, Color::Reset, "Command not found")?;
            }

        }
        
        
    }

    Ok(())
}