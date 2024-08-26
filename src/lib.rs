use astrors::{
    fits,
    io::{self, hdulist::HDUList, header::card::CardValue},
};
use core::panic;
use ndarray::{ArrayD, Axis, Ix2};
use polars::prelude::*;
use std::vec;
pub struct CcdFits {
    pub path: String,
    pub hdul: HDUList,
}

impl CcdFits {
    pub fn new(path: &str) -> Self {
        let hdul = fits::fromfile(&path).unwrap();
        CcdFits {
            path: path.to_string(),
            hdul,
        }
    }

    pub fn get_card(&self, card_name: &str) -> CardValue {
        let p_header = &self.hdul.hdus[0];
        let header = match p_header {
            io::hdulist::HDU::Primary(hdu) => &hdu.header[card_name].value,
            _ => panic!("Primary HDU not found!"),
        };
        header.clone()
    }

    fn package_polars_list(&self, data: ArrayD<u16>) -> Series {
        // convert ArrayD to Array2
        let img = match data.into_dimensionality::<Ix2>() {
            Ok(img) => img,
            Err(_) => panic!("Failed to convert ArrayD to Array2!"),
        };

        let mut chunked_builder = ListPrimitiveChunkedBuilder::<UInt16Type>::new(
            "Image",
            img.nrows(),
            img.len_of(Axis(1)),
            DataType::List(Box::new(DataType::UInt16)),
        );
        for row in img.axis_iter(Axis(0)) {
            let mut inner_builder =
                ListPrimitiveChunkedBuilder::<UInt16Type>::new("", 1, row.len(), DataType::UInt16);
            let row_vec = row.to_vec();
            inner_builder.append_slice(&row_vec);
            let inner = inner_builder.finish().into_series();
            match chunked_builder.append_series(&inner) {
                Ok(_) => (),
                Err(_) => panic!("Failed to append series!"),
            }
        }
        chunked_builder.finish().into_series()
    }

    fn get_data(&self, data: &io::hdus::image::ImageData) -> Series {
        match data {
            io::hdus::image::ImageData::U8(image) => {
                let image_data: ArrayD<u16> = image.map(|&x| x as u16);
                self.package_polars_list(image_data)
            }
            io::hdus::image::ImageData::I16(image) => {
                let image_data: ArrayD<u16> = image.map(|&x| x as u16);
                self.package_polars_list(image_data)
            }
            io::hdus::image::ImageData::I32(image) => {
                let image_data: ArrayD<u16> = image.map(|&x| x as u16);
                self.package_polars_list(image_data)
            }
            io::hdus::image::ImageData::F32(image) => {
                let image_data: ArrayD<u16> = image.map(|&x| x as u16);
                self.package_polars_list(image_data)
            }
            io::hdus::image::ImageData::F64(image) => {
                let image_data: ArrayD<u16> = image.map(|&x| x as u16);
                self.package_polars_list(image_data)
            }
            _ => panic!("Image data is not supported!"),
        }
    }

    pub fn get_image(&self) -> Series {
        let i_hdu = &self.hdul.hdus[2];
        // Match the i_hdu with the data
        let img = match i_hdu {
            io::hdulist::HDU::Image(i_hdu) => i_hdu,
            _ => panic!("Image HDU not found!"),
        };
        let image_data = self.get_data(&img.data);
        println!("{:?}", image_data);
        image_data
    }

    pub fn keys_to_polars(&self, keys: Vec<&str>) -> DataFrame {
        let mut cards = vec![];
        for key in keys {
            let val = match self.get_card(key).as_float() {
                Some(val) => val,
                None => panic!("Invalid card value!"),
            };
            let s = Series::new(key, &vec![val]);
            cards.push(s);
        }
        let img = self.get_image();
        let img_series = Series::new("Image", vec![img]);
        cards.push(img_series);
        DataFrame::new(cards).unwrap()
    }
}
