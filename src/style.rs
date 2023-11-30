use std::{io::stdout, error::Error};

use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand, 
    event,
};


pub fn print(fg:Color, bg:Color, string:&str)->Result<(), Box<dyn Error>>{
    execute!(
        stdout(),
        SetForegroundColor(fg),
        SetBackgroundColor(bg),
        Print(string),
        ResetColor
    )?;
    Ok(())
}

pub fn println(fg:Color, bg:Color, string:&str)->Result<(), Box<dyn Error>>{
    print(fg,bg,string)?;
    execute!(stdout(), ResetColor, Print("\n"))?;
    Ok(())
}