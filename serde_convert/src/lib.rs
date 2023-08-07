use anyhow::*;
use clap::*;
use serde::Serialize;
use std::fs;
use std::io::{self, Read, Write};

#[derive(Debug, Parser)]
/// Put all files in the `location` into folder with its name, multiple files that has the same
/// name (but different extension) will go into the same folder
pub struct Args {
    /// The input file or stdin if none is provided
    input: Option<String>,

    /// The input data format, will detect automatically if left empty
    #[arg(long, short, value_enum)]
    format: Option<Format>,

    /// The data format of the output
    #[arg(long, short, value_enum)]
    output: Format,

    /// Write result into a file, will write into stdout if left empty
    #[arg(long)]
    output_file: Option<String>,

    #[arg(long)]
    pretty: bool,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let mut s = String::new();

        match self.input.as_deref() {
            Some(str) => s = fs::read_to_string(str)?,
            None => {
                io::stdin().read_to_string(&mut s)?;
            }
        }

        let mut writer: Box<dyn Write> = match &self.output_file {
            Some(name) => Box::new(fs::File::create(name)?) as Box<_>,
            None => Box::new(io::stdout()) as Box<_>,
        };

        let data = parse(&s, self.format)?;

        macro_rules! typ {
            ($($x:ident),*) => {
                match (data, self.pretty) {
                    $(
                        (Data::$x(v), true) => write_pretty(&v, &mut writer, Format::$x)?,
                        (Data::$x(v), false) => write(&v, &mut writer, Format::$x)?,
                    )*
                };
            }
        }

        typ!(Json, Yaml, Toml);

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy, ValueEnum)]
enum Format {
    Json,
    Toml,
    Yaml,
}

enum Data {
    Json(serde_json::Value),
    Toml(toml::Value),
    Yaml(serde_yaml::Value),
}

fn parse(s: &str, format: Option<Format>) -> Result<Data> {
    let data = match format {
        Some(Format::Json) => serde_json::to_value(s).map(Data::Json)?,
        Some(Format::Toml) => toml::from_str::<toml::Value>(s).map(Data::Toml)?,
        Some(Format::Yaml) => serde_yaml::to_value(s).map(Data::Yaml)?,
        None => [Format::Json, Format::Toml, Format::Yaml]
            .into_iter()
            .find_map(|format| parse(s, Some(format)).ok())
            .context("Find matches format")?,
    };

    Ok(data)
}

fn write<S: Serialize, W: Write>(value: &S, writer: &mut W, format: Format) -> anyhow::Result<()> {
    match format {
        Format::Json => serde_json::to_writer(writer, value)?,
        Format::Yaml => serde_yaml::to_writer(writer, value)?,
        Format::Toml => {
            let data = toml::to_string(value)?;
            writer.write_all(data.as_bytes())?;
        }
    }

    Ok(())
}

fn write_pretty<S: Serialize, W: Write>(
    value: &S,
    writer: &mut W,
    format: Format,
) -> anyhow::Result<()> {
    match format {
        Format::Json => serde_json::to_writer_pretty(writer, value)?,
        Format::Yaml => serde_yaml::to_writer(writer, value)?,
        Format::Toml => {
            let data = toml::to_string_pretty(value)?;
            writer.write_all(data.as_bytes())?;
        }
    }

    Ok(())
}
