/// Represents different types of experiments.
pub enum ExperimentType {
    Xrr,
    Xrs,
    Other,
}

impl ExperimentType {
    /// Creates an `ExperimentType` from a string.
    pub fn from_str(exp_type: &str) -> Result<Self, crate::errors::FitsLoaderError> {
        match exp_type.to_lowercase().as_str() {
            "xrr" => Ok(ExperimentType::Xrr),
            "xrs" => Ok(ExperimentType::Xrs),
            "other" => Ok(ExperimentType::Other),
            _ => Err(crate::errors::FitsLoaderError::InvalidExperimentType(
                exp_type.to_string(),
            )),
        }
    }

    /// Retrieves the relevant header keys for the experiment type.
    pub fn get_keys(&self) -> Vec<HeaderValue> {
        match self {
            ExperimentType::Xrr => vec![
                HeaderValue::SampleTheta,
                HeaderValue::CCDTheta,
                HeaderValue::BeamlineEnergy,
                HeaderValue::BeamCurrent,
                HeaderValue::EPUPolarization,
                HeaderValue::HorizontalExitSlitSize,
                HeaderValue::HigherOrderSuppressor,
                HeaderValue::Exposure,
            ],
            ExperimentType::Xrs => vec![HeaderValue::BeamlineEnergy],
            ExperimentType::Other => vec![],
        }
    }

    /// Retrieves the header names for display purposes.
    pub fn names(&self) -> Vec<&str> {
        match self {
            ExperimentType::Xrr => vec![
                "Sample Theta",
                "CCD Theta",
                "Beamline Energy",
                "Beam Current",
                "EPU Polarization",
                "Horizontal Exit Slit Size",
                "Higher Order Suppressor",
                "EXPOSURE",
            ],
            ExperimentType::Xrs => vec!["Beamline Energy"],
            ExperimentType::Other => vec![],
        }
    }
}

/// Represents different header values.
pub enum HeaderValue {
    SampleTheta,
    CCDTheta,
    BeamlineEnergy,
    EPUPolarization,
    BeamCurrent,
    HorizontalExitSlitSize,
    HigherOrderSuppressor,
    Exposure,
}

impl HeaderValue {
    /// Returns the unit associated with the header value.
    pub fn unit(&self) -> &str {
        match self {
            HeaderValue::SampleTheta => "[deg]",
            HeaderValue::CCDTheta => "[deg]",
            HeaderValue::BeamlineEnergy => "[eV]",
            HeaderValue::BeamCurrent => "[mA]",
            HeaderValue::EPUPolarization => "[deg]",
            HeaderValue::HorizontalExitSlitSize => "[um]",
            HeaderValue::HigherOrderSuppressor => "[mm]",
            HeaderValue::Exposure => "[s]",
        }
    }

    /// Returns the HDU key associated with the header value.
    pub fn hdu(&self) -> &str {
        match self {
            HeaderValue::SampleTheta => "Sample Theta",
            HeaderValue::CCDTheta => "CCD Theta",
            HeaderValue::BeamlineEnergy => "Beamline Energy",
            HeaderValue::BeamCurrent => "Beam Current",
            HeaderValue::EPUPolarization => "EPU Polarization",
            HeaderValue::HorizontalExitSlitSize => "Horizontal Exit Slit Size",
            HeaderValue::HigherOrderSuppressor => "Higher Order Suppressor",
            HeaderValue::Exposure => "EXPOSURE",
        }
    }

    /// Returns the full name with units for display.
    pub fn name(&self) -> &str {
        match self {
            HeaderValue::SampleTheta => "Sample Theta [deg]",
            HeaderValue::CCDTheta => "CCD Theta [deg]",
            HeaderValue::BeamlineEnergy => "Beamline Energy [eV]",
            HeaderValue::BeamCurrent => "Beam Current [mA]",
            HeaderValue::EPUPolarization => "EPU Polarization [deg]",
            HeaderValue::HorizontalExitSlitSize => "Horizontal Exit Slit Size [um]",
            HeaderValue::HigherOrderSuppressor => "Higher Order Suppressor [mm]",
            HeaderValue::Exposure => "EXPOSURE [s]",
        }
    }
}
