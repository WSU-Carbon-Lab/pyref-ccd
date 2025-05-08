use astrors_fork::fits;
use astrors_fork::io::hdulist::HDU;

use polars::{lazy::prelude::*, prelude::*};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

use crate::errors::FitsLoaderError;
use crate::io::{add_calculated_domains, process_file_name, process_image, process_metadata};

/// Reads a single FITS file and converts it to a Polars DataFrame.
///
/// # Arguments
///
/// * `file_path` - Path to the FITS file to read
/// * `header_items` - List of header values to extract
///
/// # Returns
///
/// A `Result` containing either the DataFrame or a `FitsLoaderError`.
pub fn read_fits(
    file_path: std::path::PathBuf,
    header_items: &Vec<String>,
) -> Result<DataFrame, FitsLoaderError> {
    if file_path.extension().and_then(|ext| ext.to_str()) != Some("fits") {
        return Err(FitsLoaderError::NoData);
    }

    // Safely get path as string
    let path_str = file_path
        .to_str()
        .ok_or_else(|| FitsLoaderError::InvalidFileName("Invalid UTF-8 in path".into()))?;

    // Use try block pattern for more concise error handling
    let result = (|| {
        let hdul = fits::fromfile(path_str)?;

        // Process primary header metadata
        let meta = match hdul.hdus.get(0) {
            Some(HDU::Primary(hdu)) => process_metadata(hdu, header_items)?,
            _ => return Err(FitsLoaderError::NoData),
        };

        // Process image data
        let img_data = match hdul.hdus.get(2) {
            Some(HDU::Image(hdu)) => process_image(hdu)?,
            // If there's no image at index 2, try index 1 as a fallback
            _ => match hdul.hdus.get(1) {
                Some(HDU::Image(hdu)) => process_image(hdu)?,
                _ => return Err(FitsLoaderError::NoData),
            },
        };

        // Extract file name information
        let names = process_file_name(file_path.clone());

        // Combine all columns
        let mut columns = meta;
        columns.extend(img_data);
        columns.extend(names);

        // Create DataFrame
        DataFrame::new(columns).map_err(FitsLoaderError::PolarsError)
    })();

    // Add file path to error context if an error occurred
    result.map_err(|e| {
        FitsLoaderError::FitsError(format!("Error processing file '{}': {}", path_str, e))
    })
}

/// Helper function to combine DataFrames with schema alignment
fn combine_dataframes_with_alignment(
    acc: DataFrame,
    df: DataFrame,
) -> Result<DataFrame, FitsLoaderError> {
    // Try simple vstack first
    match acc.vstack(&df) {
        Ok(combined) => Ok(combined),
        Err(_) => {
            // If vstack fails, align the schemas and try again
            let acc_cols = acc.get_column_names();
            let df_cols = df.get_column_names();

            // Find missing columns in each DataFrame
            let missing_in_acc: Vec<_> = df_cols.iter().filter(|c| !acc_cols.contains(c)).collect();
            let missing_in_df: Vec<_> = acc_cols.iter().filter(|c| !df_cols.contains(c)).collect();

            // Add missing columns to each DataFrame with null values
            let mut acc_aligned = acc.clone();
            let mut df_aligned = df.clone();

            for col in missing_in_acc {
                // Convert to PlSmallStr
                let col_name: PlSmallStr = (*col).clone().into();
                let null_series = Series::new_null(col_name, acc.height());
                let _ = acc_aligned.with_column(null_series).unwrap();
            }

            for col in missing_in_df {
                // Convert to PlSmallStr
                let col_name: PlSmallStr = (*col).clone().into();
                let null_series = Series::new_null(col_name, df.height());
                let _ = df_aligned.with_column(null_series).unwrap();
            }

            // Try again with aligned schemas
            acc_aligned
                .vstack(&df_aligned)
                .map_err(|e| FitsLoaderError::PolarsError(e))
        }
    }
}

/// Reads all FITS files in a directory and combines them into a single DataFrame.
///
/// # Arguments
///
/// * `dir` - Path to the directory containing FITS files
/// * `header_items` - List of header values to extract
///
/// # Returns
///
/// A `Result` containing either the combined DataFrame or a `FitsLoaderError`.
pub fn read_experiment(
    dir: &str,
    header_items: &Vec<String>,
) -> Result<DataFrame, FitsLoaderError> {
    let dir_path = std::path::PathBuf::from(dir);

    if !dir_path.exists() {
        return Err(FitsLoaderError::FitsError(format!(
            "Directory not found: {}",
            dir
        )));
    }

    // Find all FITS files in the directory
    let entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| FitsLoaderError::IoError(e))?
        .par_bridge()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("fits"))
        .collect();

    if entries.is_empty() {
        return Err(FitsLoaderError::FitsError(format!(
            "No FITS files found in directory: {}",
            dir
        )));
    }

    // Process each file in parallel, collect results
    let results: Vec<Result<DataFrame, FitsLoaderError>> = entries
        .par_iter()
        .map(|entry| read_fits(entry.path(), &header_items))
        .collect();

    // Filter out errors and keep only successful DataFrames
    let successful_dfs: Vec<DataFrame> = results
        .into_iter()
        .filter_map(|result| result.ok())
        .collect();

    // If no files were successfully processed, return an error
    if successful_dfs.is_empty() {
        return Err(FitsLoaderError::FitsError(
            "None of the files in the directory could be processed successfully".into(),
        ));
    }

    // Combine all successful DataFrames
    let combined_df = successful_dfs
        .into_par_iter()
        .reduce_with(|acc, df| {
            let acc_clone = acc.clone();
            combine_dataframes_with_alignment(acc, df).unwrap_or(acc_clone)
        })
        .ok_or(FitsLoaderError::NoData)?;

    // If there is a column for energy, theta add the q column
    Ok(add_calculated_domains(combined_df.lazy()))
}

/// Reads multiple specific FITS files and combines them into a single DataFrame.
///
/// # Arguments
///
/// * `file_paths` - Vector of paths to the FITS files to read
/// * `header_items` - List of header values to extract
///
/// # Returns
///
/// A `Result` containing either the combined DataFrame or a `FitsLoaderError`.
pub fn read_multiple_fits(
    file_paths: Vec<PathBuf>,
    header_items: &Vec<String>,
) -> Result<DataFrame, FitsLoaderError> {
    if file_paths.is_empty() {
        return Err(FitsLoaderError::FitsError("No files provided".into()));
    }

    // Check that all files exist
    for path in &file_paths {
        if !path.exists() {
            return Err(FitsLoaderError::FitsError(format!(
                "File not found: {}",
                path.display()
            )));
        }
    }

    // Process each file in parallel, collect results
    let results: Vec<Result<DataFrame, FitsLoaderError>> = file_paths
        .par_iter()
        .map(|path| read_fits(path.clone(), header_items))
        .collect();

    // Filter out errors and keep only successful DataFrames
    let successful_dfs: Vec<DataFrame> = results
        .into_iter()
        .filter_map(|result| result.ok())
        .collect();

    // If no files were successfully processed, return an error
    if successful_dfs.is_empty() {
        return Err(FitsLoaderError::FitsError(
            "None of the provided files could be processed successfully".into(),
        ));
    }

    // Combine all successful DataFrames
    let combined_df = successful_dfs
        .into_par_iter()
        .reduce_with(|acc, df| {
            let acc_clone = acc.clone();
            combine_dataframes_with_alignment(acc, df).unwrap_or(acc_clone)
        })
        .ok_or(FitsLoaderError::NoData)?;

    Ok(add_calculated_domains(combined_df.lazy()))
}

/// Reads FITS files matching a pattern and combines them into a single DataFrame.
///
/// # Arguments
///
/// * `dir` - Directory containing FITS files
/// * `pattern` - Glob pattern to match files (e.g., "Y6_refl_*.fits")
/// * `header_items` - List of header values to extract
///
/// # Returns
///
/// A `Result` containing either the combined DataFrame or a `FitsLoaderError`.
pub fn read_experiment_pattern(
    dir: &str,
    pattern: &str,
    header_items: &Vec<String>,
) -> Result<DataFrame, FitsLoaderError> {
    let dir_path = std::path::PathBuf::from(dir);

    if !dir_path.exists() {
        return Err(FitsLoaderError::FitsError(format!(
            "Directory not found: {}",
            dir
        )));
    }

    // Clone the header items to avoid borrowing issues
    let header_items = header_items
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    // Find all matching FITS files
    let entries: Vec<_> = fs::read_dir(dir)
        .map_err(FitsLoaderError::IoError)?
        .par_bridge()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.extension().and_then(|ext| ext.to_str()) == Some("fits")
                && match path.file_name().and_then(|name| name.to_str()) {
                    Some(name) => glob_match::glob_match(pattern, name),
                    None => false,
                }
        })
        .map(|entry| entry.path())
        .collect();

    if entries.is_empty() {
        return Err(FitsLoaderError::FitsError(format!(
            "No FITS files matching pattern '{}' found in directory: {}",
            pattern, dir
        )));
    }

    read_multiple_fits(entries, &header_items)
}
