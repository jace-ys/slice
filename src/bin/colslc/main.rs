use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process;

use clap::Clap;

use slices::filter::{Filter, FilterSet};

#[derive(Clap)]
#[clap(
    name = "colslc",
    version = "1.0.0",
    author = "Jace Tan <jaceys.tan@gmail.com>"
)]
struct Opts {
    /// Path to input file
    filepath: Option<String>,

    #[clap(short, long)]
    /// Filters to be applied
    filters: Vec<Filter>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();

    let reader: Box<dyn BufRead> = match opts.filepath.as_deref() {
        Some("-") => Box::new(BufReader::new(io::stdin())),
        Some(input) => {
            let file = File::open(input)
                .map_err(|err| format!("failed to open file {}: {}", input, err))?;
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };
    let mut writer = BufWriter::new(io::stdout());

    let mut slicer = ColSlicer {
        reader,
        filters: FilterSet::new(opts.filters),
    };

    slicer
        .slice(&mut writer)
        .map_err(|err| format!("slice operation failed: {}", err))?;

    Ok(())
}

struct ColSlicer<R: BufRead> {
    reader: R,
    filters: FilterSet,
}

impl<R: BufRead> ColSlicer<R> {
    fn slice<W: Write>(&mut self, writer: &mut W) -> Result<(), Box<dyn Error>> {
        let mut buf = String::new();

        loop {
            match self.reader.read_line(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if self.filters.is_empty() {
                        write!(writer, "{}", buf)?;
                    } else {
                        let columns: Vec<&str> = buf
                            .split_whitespace()
                            .enumerate()
                            .filter(|&(index, _)| self.filters.apply(1 + index as u32))
                            .map(|(_, col)| col)
                            .collect();

                        writeln!(writer, "{}", columns.join(" "))?;
                    }

                    buf.clear();
                }
                Err(err) => return Err(err.into()),
            }
        }

        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    fn testdata() -> File {
        File::open("src/testdata/input.txt").unwrap()
    }

    #[test]
    fn colslc_slice_exact_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("1")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY
vault
redis
postgres
traefik
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_exact_multiple_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("1")?, Filter::from_str("3")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY IMAGE
vault dc15db720d79
redis 6960a2858b36
postgres ae192c4d3ada
traefik 72bfc37343a4
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_range_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("1:3")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY TAG IMAGE
vault 1.8.4 dc15db720d79
redis 6.2-alpine 6960a2858b36
postgres 14.0-alpine ae192c4d3ada
traefik 2.5 72bfc37343a4
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_range_multiple_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("1:2")?, Filter::from_str("4:5")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY TAG ID CREATED
vault 1.8.4 2 days
redis 6.2-alpine 3 days
postgres 14.0-alpine 17 months
traefik 2.5 18 months
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_exact_and_range_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("1")?, Filter::from_str("3:4")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY IMAGE ID
vault dc15db720d79 2
redis 6960a2858b36 3
postgres ae192c4d3ada 17
traefik 72bfc37343a4 18
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_range_start_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str("3:")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
IMAGE ID CREATED SIZE
dc15db720d79 2 days ago 186MB
6960a2858b36 3 days ago 31.3MB
ae192c4d3ada 17 months ago 152MB
72bfc37343a4 18 months ago 68.9MB
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_range_end_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str(":3")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY TAG IMAGE
vault 1.8.4 dc15db720d79
redis 6.2-alpine 6960a2858b36
postgres 14.0-alpine ae192c4d3ada
traefik 2.5 72bfc37343a4
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }

    #[test]
    fn colslc_slice_range_full_ok() -> Result<(), Box<dyn Error>> {
        let mut writer = Vec::new();

        let filters = vec![Filter::from_str(":")?];
        let mut slicer = ColSlicer {
            reader: BufReader::new(testdata()),
            filters: FilterSet::new(filters),
        };

        let expected = "\
REPOSITORY TAG IMAGE ID CREATED SIZE
vault 1.8.4 dc15db720d79 2 days ago 186MB
redis 6.2-alpine 6960a2858b36 3 days ago 31.3MB
postgres 14.0-alpine ae192c4d3ada 17 months ago 152MB
traefik 2.5 72bfc37343a4 18 months ago 68.9MB
";

        slicer.slice(&mut writer)?;
        assert_eq!(String::from_utf8(writer)?, expected);
        Ok(())
    }
}
