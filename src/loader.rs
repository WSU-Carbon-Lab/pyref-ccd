/// This module provides functionality for working with FITS files using the `astrors::fits` crate.
///
/// # Examples
///
/// ```
/// use astrors::fits;
///
/// // Load a FITS file
/// let fits_file = fits::load("path/to/file.fits");
///
/// // Access the header information
/// let header = fits_file.header();
///
/// // Access the data
/// let data = fits_file.data();
/// ```
///
/// For more information, see the [README](README.md).
use astrors_fork::fits;
use astrors_fork::io;
use astrors_fork::io::hdulist::*;
use astrors_fork::io::header::*;
// use jemallocator::Jemalloc;
use numpy::ndarray::{aview1, Array2};
use physical_constants;
use polars::prelude::*;
use rayon::prelude::*;
use std::fs;

// #[global_allocator]
// static GLOBAL: Jemalloc = Jemalloc;

// Enum representing different types of experiments.
pub enum ExperimentType {
    Xrr,
    Xrs,
    Other,
}

impl ExperimentType {
    pub fn from_str(exp_type: &str) -> Result<Self, &str> {
        match exp_type.to_lowercase().as_str() {
            "xrr" => Ok(ExperimentType::Xrr),
            "xrs" => Ok(ExperimentType::Xrs),
            "other" => Ok(ExperimentType::Other),
            _ => Err("Invalid experiment type"),
        }
    }

    pub fn get_keys(&self) -> Vec<&str> {
        match self {
            ExperimentType::Xrr => vec![
                "Sample Theta",
                "Beamline Energy",
                "EPU Polarization",
                "Horizontal Exit Slit Size",
                "Higher Order Suppressor",
                "EXPOSURE",
            ],
            ExperimentType::Xrs => vec!["Energy"],
            ExperimentType::Other => vec![],
        }
    }
}

// Struct representing a CCD FITS file.
pub struct FitsLoader {
    pub path: String,
    pub hdul: HDUList,
}

/// FitsLoader struct for loading and accessing FITS file data.
///
/// The `FitsLoader` struct provides methods for loading and accessing data from a FITS file.
/// It supports retrieving individual card values, all card values, image data, and converting
/// the data to a Polars DataFrame.
///
/// # Example
///
/// ```
/// extern crate pyref_core;
/// use pyref_core::loader::FitsLoader;
///
/// let fits_loader = FitsLoader::new("/path/to/file.fits").unwrap();
///
/// // Get a specific card value
/// let card_value = fits_loader.get_value("CARD_NAME");
///
/// // Get all card values
/// let all_cards = fits_loader.get_all_cards();
///
/// // Get image data
/// let image_data = fits_loader.get_image();
///
/// // Convert data to Polars DataFrame
/// let keys = ["KEY1", "KEY2"];
/// let polars_df = fits_loader.to_polars(&keys);
/// ```
/// A struct representing a FITS loader.

impl FitsLoader {
    /// Creates a new `FitsLoader` instance.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the FITS file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `FitsLoader` instance if successful, or a boxed `dyn std::error::Error` if an error occurred.
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let hdul = fits::fromfile(path)?;
        Ok(FitsLoader {
            path: path.to_string(),
            hdul,
        })
    }

    /// Retrieves a specific card from the FITS file.
    ///
    /// # Arguments
    ///
    /// * `card_name` - The name of the card to retrieve.
    ///
    /// # Returns
    ///
    /// An `Option` containing the requested `card::Card` if found, or `None` if not found.
    pub fn get_card(&self, card_name: &str) -> Option<card::Card> {
        match &self.hdul.hdus[0] {
            io::hdulist::HDU::Primary(hdu) => hdu.header.get_card(card_name).cloned(),
            _ => None,
        }
    }

    /// Retrieves the value of a specific card from the FITS file.
    ///
    /// # Arguments
    ///
    /// * `card_name` - The name of the card to retrieve the value from.
    ///
    /// # Returns
    ///
    /// An `Option` containing the value of the requested card as a `f64` if found, or `None` if not found.
    pub fn get_value(&self, card_name: &str) -> Option<f64> {
        if card_name == "Q" {
            let theta = self.get_value("Sample Theta");
            let en = self.get_value("Beamline Energy");
            // calculate the q value from the sample theta and beamline energy
            let lambda = 1e10
                * physical_constants::MOLAR_PLANCK_CONSTANT
                * physical_constants::SPEED_OF_LIGHT_IN_VACUUM
                / en.unwrap();
            return Some(4.0 * std::f64::consts::PI * (theta.unwrap().to_radians().sin() / lambda));
        }
        match &self.hdul.hdus[0] {
            io::hdulist::HDU::Primary(hdu) => hdu
                .header
                .get_card(card_name)
                .map(|c| c.value.as_float().unwrap()),
            _ => None,
        }
    }

    /// Retrieves all cards from the FITS file.
    ///
    /// # Returns
    ///
    /// A `Vec` containing all the cards as `card::Card` instances.
    pub fn get_all_cards(&self) -> Vec<card::Card> {
        match &self.hdul.hdus[0] {
            io::hdulist::HDU::Primary(hdu) => {
                hdu.header.iter().cloned().collect::<Vec<card::Card>>()
            }
            _ => vec![],
        }
    }

    /// Retrieves the image data from the FITS file.
    ///
    /// # Arguments
    ///
    /// * `data` - The image data to retrieve.
    ///
    /// # Returns
    ///
    /// A `Result` containing the image data as a `Array2<u32>` if successful, or a boxed `dyn std::error::Error` if an error occurred.
    fn get_data(
        &self,
        data: &io::hdus::image::ImageData,
    ) -> Result<(Vec<u32>, Vec<u32>), Box<dyn std::error::Error + Send + Sync>> {
        let (flat_data, shape) = match data {
            io::hdus::image::ImageData::I16(image) => {
                let flat_data = image.iter().map(|&x| u32::from(x as u16)).collect();
                let shape = image.dim();
                (flat_data, shape)
            }
            _ => return Err("Unsupported image data type".into()),
        };
        Ok((flat_data, vec![shape[0] as u32, shape[1] as u32]))
    }

    /// Retrieves the image data from the FITS file as an `Array2<u32>`.
    ///
    /// # Returns
    ///
    /// A `Result` containing the image data as a `Array2<u32>` if successful, or a boxed `dyn std::error::Error` if an error occurred.
    pub fn get_image(
        &self,
    ) -> Result<(Vec<u32>, Vec<u32>), Box<dyn std::error::Error + Send + Sync>> {
        match &self.hdul.hdus[2] {
            io::hdulist::HDU::Image(i_hdu) => self.get_data(&i_hdu.data),
            _ => Err("Image HDU not found".into()),
        }
    }

    /// Converts the FITS file data to a `polars::prelude::DataFrame`.
    ///
    /// # Arguments
    ///
    /// * `keys` - The keys of the cards to include in the DataFrame. If empty, all cards will be included.
    ///
    /// # Returns
    ///
    /// A `Result` containing the converted `DataFrame` if successful, or a boxed `dyn std::error::Error` if an error occurred.
    pub fn to_polars(
        &self,
        keys: &[&str],
    ) -> Result<DataFrame, Box<dyn std::error::Error + Send + Sync>> {
        let mut s_vec = if keys.is_empty() {
            // When keys are empty, use all cards.
            self.get_all_cards()
                .iter()
                .map(|card| {
                    let name = card.keyword.as_str();
                    let value = card.value.as_float().unwrap_or(0.0);
                    Series::new(name, vec![value])
                })
                .collect::<Vec<_>>()
        } else {
            // Use specified keys
            keys.iter()
                .filter_map(|key| {
                    self.get_value(key)
                        .map(|value| Series::new(key, vec![value]))
                })
                .collect::<Vec<_>>()
        };
        // Add the image data
        let (image, size) = match self.get_image() {
            Ok(data) => data,
            Err(e) => return Err(e),
        };
        s_vec.push(vec_series("Raw", image));
        s_vec.push(vec_series("Raw Shape", size));
        s_vec.push(Series::new("Q [A^-1]", vec![self.get_value("Q").unwrap()]));
        DataFrame::new(s_vec).map_err(From::from)
    }
}
// Function facilitate storing the image data as a single element in a Polars DataFrame.
pub fn vec_series(name: &str, img: Vec<u32>) -> Series {
    let new_series = [img.iter().collect::<Series>()];
    Series::new(name, new_series)
}

pub struct ExperimentLoader {
    pub dir: String,
    pub ccd_files: Vec<FitsLoader>,
    pub experiment_type: ExperimentType,
}

/// FitsLoader struct for loading and accessing FITS file data.
///
/// The `FitsLoader` struct provides methods for loading and accessing data from a FITS file.
/// It supports retrieving individual card values, all card values, image data, and converting
/// the data to a Polars DataFrame.
///
/// # Example
///
/// ```
/// extern crate pyref_core;
/// use pyref_core::loader::{ExperimentLoader, ExperimentType};
///
/// let exp = ExperimentType::from_str(exp_type)?;
/// let fits_loader = ExperimentLoader::new("/path/to/file.fits", exp).unwrap();
///
/// // Mostly this is used to convert the data to a Polars DataFrame
/// let df = fits_loader.to_polars()?;
/// ```

impl ExperimentLoader {
    // Create a new ExperimentLoader instance and load all Fits file in the directory.
    pub fn new(
        dir: &str,
        experiment_type: ExperimentType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let ccd_files: Vec<_> = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("fits"))
            .collect();

        let ccd_files = ccd_files
            .par_iter() // Parallel iterator using Rayon
            .map(|entry| FitsLoader::new(entry.path().to_str().unwrap()))
            .collect::<Result<Vec<_>, Box<dyn std::error::Error + Send + Sync>>>();
        let ccd_files = match ccd_files {
            Ok(ccd_files) => ccd_files,
            Err(e) => return Err(e),
        };

        Ok(ExperimentLoader {
            dir: dir.to_string(),
            ccd_files,
            experiment_type,
        })
    }
    // Package all loaded FITS files into a single Polars DataFrame.
    pub fn to_polars(&self) -> Result<DataFrame, Box<dyn std::error::Error>> {
        let keys = self.experiment_type.get_keys();

        let dfs = self
            .ccd_files
            .par_iter()
            .map(|ccd| ccd.to_polars(&keys))
            .collect::<Result<Vec<_>, _>>();
        let mut dfs = match dfs {
            Ok(dfs) => dfs,
            Err(e) => return Err(e),
        };
        let mut df = dfs.pop().ok_or("No data found")?;
        for mut d in dfs {
            df.vstack_mut(&mut d)?;
        }
        Ok(df)
    }
}

// function to unpack an image wile iterating rhough a polars dataframe.
pub fn get_image(vec: Vec<u32>, shape: Vec<u32>) -> Array2<u32> {
    let (rows, cols) = (shape[0] as usize, shape[1] as usize);
    let img = aview1(&vec.clone()).to_owned();
    img.into_shape((rows, cols)).unwrap()
}

// workhorse functions for loading and processing CCD data.
pub fn read_fits(file_path: &str) -> Result<DataFrame, Box<dyn std::error::Error>> {
    let loader = match FitsLoader::new(file_path) {
        Ok(loader) => loader,
        Err(e) => return Err(e),
    };
    let df = match loader.to_polars(&[]) {
        Ok(df) => df,
        Err(e) => return Err(e),
    };
    Ok(df)
}

pub fn read_experiment(dir: &str, exp_type: &str) -> Result<DataFrame, Box<dyn std::error::Error>> {
    let exp = ExperimentType::from_str(exp_type)?;
    let df = ExperimentLoader::new(dir, exp)?.to_polars()?;
    Ok(df)
}

pub fn simple_update(df: &mut DataFrame, dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ccd_files: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("fits"))
        .collect();
    let not_loaded = ccd_files.len() as isize - df.height() as isize;
    if not_loaded == 0 {
        return Ok(());
    } else if not_loaded < 0 {
        return Err("Files out of sync with loaded data, Restart".into());
    }
    let ccd_files = ccd_files[..not_loaded as usize]
        .par_iter() // Parallel iterator using Rayon
        .map(|entry| FitsLoader::new(entry.path().to_str().unwrap()))
        .collect::<Result<Vec<_>, Box<dyn std::error::Error + Send + Sync>>>();
    let ccd_files = match ccd_files {
        Ok(ccd_files) => ccd_files,
        Err(e) => return Err(e),
    };
    let mut new_df = ExperimentLoader {
        dir: dir.to_string(),
        ccd_files,
        experiment_type: ExperimentType::Xrr,
    }
    .to_polars()?;
    df.vstack_mut(&mut new_df)?;
    Ok(())
}
