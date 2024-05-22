// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! The sysfs backend

use mockall_double::double;
use regex::Regex;

use std::path::Path;
use std::path::PathBuf;

use crate::pd::PdPdo;
use crate::ucsi::ConnectorCapabilityOperationMode;
use crate::ucsi::GetAlternateModesRecipient;
use crate::ucsi::PdMessage;
use crate::ucsi::PdMessageRecipient;
use crate::ucsi::PdMessageResponseType;
use crate::ucsi::PdoSourceCapabilitiesType;
use crate::ucsi::PdoType;
use crate::ucsi::UcsiAlternateMode;
use crate::ucsi::UcsiCableProperty;
use crate::ucsi::UcsiCapability;
use crate::ucsi::UcsiConnectorCapability;
use crate::ucsi::UcsiConnectorStatus;
use crate::BcdWrapper;
use crate::Error;
use crate::OsBackend;
use crate::Result;

#[double]
use sysfs_reader::SysfsReader;
#[double]
use sysfs_walker::SysfsWalker;

const SYSFS_TYPEC_PATH: &str = "/sys/class/typec";
const SYSFS_PSY_PATH: &str = "/sys/class/power_supply";

/// Creates a `PathBuf` from a string and returns an error if the path does not
/// exist.
fn check_path(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if !path.exists() {
        Err(Error::NotSupported {
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
        })
    } else {
        Ok(path)
    }
}

/// A module to differentiate `SysfsReader` from `MockSysfsReader`. This is a
/// limitation of the `mockall` crate.
pub mod sysfs_reader {
    #[cfg(test)]
    use mockall::{automock, predicate::*};

    use std::io;
    use std::io::Cursor;
    use std::path::Path;
    use std::path::PathBuf;

    use crate::pd::Pd3p2BatterySupplyPdo;
    use crate::pd::Pd3p2DiscoverIdentityResponse;
    use crate::pd::Pd3p2FastRoleSwap;
    use crate::pd::Pd3p2FixedSupplyPdo;
    use crate::pd::Pd3p2SprProgrammableSupplyPdo;
    use crate::pd::Pd3p2VariableSupplyPdo;
    use crate::ucsi::CablePropertyPlugEndType;
    use crate::ucsi::CablePropertyType;
    use crate::ucsi::ConnectorCapabilityOperationMode;
    use crate::ucsi::PdMessageRecipient;
    use crate::ucsi::PdoType;
    use crate::vdo::Pd3p2CertStatVdo;
    use crate::vdo::Pd3p2IdHeaderVdo;
    use crate::vdo::Pd3p2ProductTypeVdo;
    use crate::vdo::Pd3p2ProductVdo;
    use crate::BcdWrapper;
    use crate::BitReader;
    use crate::Error;
    use crate::FromBytes;
    use crate::Result;

    use super::SYSFS_TYPEC_PATH;

    /// A mockable sysfs reader.
    pub struct SysfsReader(Option<PathBuf>);

    #[cfg_attr(test, automock)]
    impl SysfsReader {
        pub fn new() -> Result<Self> {
            Ok(Self(None))
        }

        pub fn set_path(&mut self, path: &str) -> Result<()> {
            self.0 = Some(super::check_path(path)?);
            Ok(())
        }

        fn read_file(&mut self) -> Result<String> {
            let path = self.0.take().expect("Path not set");
            let string = std::fs::read_to_string(path)?;
            Ok(string)
        }

        pub fn read_bcd(&mut self) -> Result<BcdWrapper> {
            let content = self.read_file()?;
            let mut chars = content.chars();

            let high = chars
                .next()
                .ok_or(io::Error::new(io::ErrorKind::InvalidData, "File is empty"))?;
            let _ = chars.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "File is too short",
            ))?;

            // Sometimes we get simply "2"
            let low = chars.next().unwrap_or('0');

            let high = high.to_digit(10).ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid digit: {high}"),
            ))?;
            let low = low.to_digit(10).ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid digit: {low}"),
            ))?;

            let bcd = (high << 8) | low;

            Ok(BcdWrapper(bcd))
        }

        pub fn read_opr(&mut self) -> Result<ConnectorCapabilityOperationMode> {
            let content = self.read_file()?;
            if content.contains("source") {
                if content.contains("sink") {
                    Ok(ConnectorCapabilityOperationMode::Drp)
                } else {
                    Ok(ConnectorCapabilityOperationMode::RpOnly)
                }
            } else {
                Ok(ConnectorCapabilityOperationMode::RdOnly)
            }
        }

        pub fn read_pd_revision(&mut self) -> Result<u8> {
            let content = self.read_file()?;
            let mut chars = content.chars();

            let b0 = chars.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "File is too short",
            ))?;
            let _ = chars.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "File is too short",
            ))?;
            let b2 = chars.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "File is too short",
            ))?;

            let rev = ((b0 as u8 - b'0' as u8) << 4) | (b2 as u8 - b'0' as u8);
            Ok(rev)
        }

        pub fn read_hex_u32(&mut self) -> Result<u32> {
            let content = self.read_file()?.replace("0x", "");
            let hex = u32::from_str_radix(content.trim(), 16).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Could not parse hex value")
            })?;
            Ok(hex)
        }

        pub fn read_u32(&mut self) -> Result<u32> {
            let mut content = self.read_file()?;
            content.retain(|c| c.is_ascii_digit());

            let dword = content.trim().parse::<u32>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Could not parse u32 value")
            })?;
            Ok(dword)
        }

        pub fn read_bit(&mut self) -> Result<bool> {
            let content = self.read_file()?;
            let bit = content.trim().parse::<bool>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Could not parse bool value")
            })?;
            Ok(bit)
        }

        pub fn read_cable_plug_type(&mut self) -> Result<CablePropertyPlugEndType> {
            let content = self.read_file()?;
            let plug_type = if content.contains("type-c") {
                CablePropertyPlugEndType::UsbTypeC
            } else if content.contains("type-a") {
                CablePropertyPlugEndType::UsbTypeA
            } else if content.contains("type-b") {
                CablePropertyPlugEndType::UsbTypeB
            } else {
                CablePropertyPlugEndType::OtherNotUsb
            };

            Ok(plug_type)
        }

        pub fn read_cable_type(&mut self) -> Result<CablePropertyType> {
            let content = self.read_file()?;
            let cable_type = if content.contains("active") {
                CablePropertyType::Active
            } else if content.contains("passive") {
                CablePropertyType::Passive
            } else {
                return Err(Error::ParseStringError {
                    field: "cable_type".to_string(),
                    value: content,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                });
            };

            Ok(cable_type)
        }

        pub fn read_cable_mode_support(&mut self) -> Result<bool> {
            let content = self.read_file()?;
            let mode_support = match content.chars().next() {
                Some('0') => false,
                Some(_) => true,
                None => {
                    return Err(Error::ParseStringError {
                        field: "cable_mode_support".to_string(),
                        value: content,
                        #[cfg(feature = "backtrace")]
                        backtrace: std::backtrace::Backtrace::capture(),
                    });
                }
            };

            Ok(mode_support)
        }

        pub fn read_fixed_supply_pdo(
            &mut self,
            path: &Path,
            src_or_sink: PdoType,
        ) -> Result<Pd3p2FixedSupplyPdo> {
            match src_or_sink {
                PdoType::Source => {
                    self.set_path(&path.join("dual_role_power").to_string_lossy())?;
                    let dual_role_power = self.read_bit()?;
                    self.set_path(&path.join("higher_capability").to_string_lossy())?;
                    let higher_capability = self.read_bit()?;
                    self.set_path(&path.join("unconstrained_power").to_string_lossy())?;
                    let unconstrained_power = self.read_bit()?;
                    self.set_path(&path.join("usb_communication_capable").to_string_lossy())?;
                    let usb_communications_capable = self.read_bit()?;
                    self.set_path(&path.join("dual_role_data").to_string_lossy())?;
                    let dual_role_data = self.read_bit()?;
                    self.set_path(&path.join("fast_role_swap").to_string_lossy())?;
                    let fast_role_swap = self.read_u32()?;
                    let fast_role_swap =
                        Pd3p2FastRoleSwap::n(fast_role_swap).ok_or_else(|| Error::ParseError {
                            field: "fast_role_swap".into(),
                            value: fast_role_swap,
                            #[cfg(feature = "backtrace")]
                            backtrace: std::backtrace::Backtrace::capture(),
                        })?;
                    self.set_path(&path.join("voltage").to_string_lossy())?;
                    let voltage = (self.read_u32()? / 50).into();
                    self.set_path(&path.join("maximum_current").to_string_lossy())?;
                    let operational_current = (self.read_u32()? / 10).into();

                    Ok(Pd3p2FixedSupplyPdo {
                        dual_role_power,
                        higher_capability,
                        unconstrained_power,
                        usb_communications_capable,
                        dual_role_data,
                        fast_role_swap,
                        voltage,
                        operational_current,
                    })
                }
                PdoType::Sink => {
                    self.set_path(&path.join("dual_role_power").to_string_lossy())?;
                    let dual_role_power = self.read_bit()?;
                    self.set_path(&path.join("higher_capability").to_string_lossy())?;
                    let higher_capability = self.read_bit()?;
                    self.set_path(&path.join("unconstrained_power").to_string_lossy())?;
                    let unconstrained_power = self.read_bit()?;
                    self.set_path(&path.join("usb_communication_capable").to_string_lossy())?;
                    let usb_communications_capable = self.read_bit()?;
                    self.set_path(&path.join("dual_role_data").to_string_lossy())?;
                    let dual_role_data = self.read_bit()?;
                    self.set_path(&path.join("fast_role_swap_current").to_string_lossy())?;
                    let fast_role_swap = self.read_u32()?;
                    let fast_role_swap =
                        Pd3p2FastRoleSwap::n(fast_role_swap).ok_or_else(|| Error::ParseError {
                            field: "fast_role_swap".into(),
                            value: fast_role_swap,
                            #[cfg(feature = "backtrace")]
                            backtrace: std::backtrace::Backtrace::capture(),
                        })?;
                    self.set_path(&path.join("voltage").to_string_lossy())?;
                    let voltage = (self.read_u32()? / 50).into();
                    self.set_path(&path.join("operational_current").to_string_lossy())?;
                    let operational_current = (self.read_u32()? / 10).into();

                    Ok(Pd3p2FixedSupplyPdo {
                        dual_role_power,
                        higher_capability,
                        unconstrained_power,
                        usb_communications_capable,
                        dual_role_data,
                        fast_role_swap,
                        voltage,
                        operational_current,
                    })
                }
            }
        }

        pub fn read_programmable_supply_pdo(
            &mut self,
            path: &Path,
            src_or_sink: PdoType,
        ) -> Result<Pd3p2SprProgrammableSupplyPdo> {
            self.set_path(&path.join("maximum_voltage").to_string_lossy())?;
            let max_voltage = (self.read_u32()? / 50).into();
            self.set_path(&path.join("minimum_voltage").to_string_lossy())?;
            let min_voltage = (self.read_u32()? / 50).into();
            let max_current = (match src_or_sink {
                PdoType::Source => {
                    self.set_path(&path.join("maximum_current").to_string_lossy())?;
                    self.read_u32()?
                }
                PdoType::Sink => {
                    self.set_path(&path.join("operational_current").to_string_lossy())?;
                    self.read_u32()?
                }
            } / 10)
                .into();

            Ok(Pd3p2SprProgrammableSupplyPdo {
                max_voltage,
                min_voltage,
                max_current,
            })
        }

        pub fn read_battery_supply_pdo(
            &mut self,
            path: &Path,
            src_or_sink: PdoType,
        ) -> Result<Pd3p2BatterySupplyPdo> {
            self.set_path(&path.join("maximum_voltage").to_string_lossy())?;
            let max_voltage = (self.read_u32()? / 50).into();
            self.set_path(&path.join("minimum_voltage").to_string_lossy())?;
            let min_voltage = (self.read_u32()? / 50).into();
            let operational_power = (match src_or_sink {
                PdoType::Source => {
                    self.set_path(&path.join("maximum_power").to_string_lossy())?;
                    self.read_u32()?
                }
                PdoType::Sink => {
                    self.set_path(&path.join("operational_power").to_string_lossy())?;
                    self.read_u32()?
                }
            } / 250)
                .into();

            Ok(Pd3p2BatterySupplyPdo {
                max_voltage,
                min_voltage,
                operational_power,
            })
        }

        pub fn read_variable_supply_pdo(
            &mut self,
            path: &Path,
            _src_or_sink: PdoType,
        ) -> Result<Pd3p2VariableSupplyPdo> {
            self.set_path(&path.join("maximum_voltage").to_string_lossy())?;
            let max_voltage = (self.read_u32()? / 100).into();
            self.set_path(&path.join("minimum_voltage").to_string_lossy())?;
            let min_voltage = (self.read_u32()? / 100).into();
            self.set_path(&path.join("maximum_current").to_string_lossy())?;
            let max_current = (self.read_u32()? / 50).into();

            Ok(Pd3p2VariableSupplyPdo {
                max_voltage,
                min_voltage,
                max_current,
            })
        }

        pub fn discover_identity(
            &mut self,
            conn_num: usize,
            recipient: PdMessageRecipient,
        ) -> Result<Pd3p2DiscoverIdentityResponse> {
            let (cert_stat, id_header, product, product_type_vdo) = match recipient {
                PdMessageRecipient::Sop => {
                    let path_str =
                        format!("{}/port{}-partner/identity", SYSFS_TYPEC_PATH, conn_num);
                    self.read_identity(&path_str)?
                }
                PdMessageRecipient::SopPrime => {
                    let path_str = format!("{}/port{}-cable/identity", SYSFS_TYPEC_PATH, conn_num);
                    self.read_identity(&path_str)?
                }
                _ => {
                    return Err(Error::NotSupported {
                        #[cfg(feature = "backtrace")]
                        backtrace: std::backtrace::Backtrace::capture(),
                    })
                }
            };

            let binding = id_header.to_le_bytes();
            let mut br = BitReader::new(Cursor::new(&binding));
            let id_header_vdo = Pd3p2IdHeaderVdo::from_bytes(&mut br)?;

            let binding = cert_stat.to_le_bytes();
            let mut br = BitReader::new(Cursor::new(&binding));
            let cert_stat = Pd3p2CertStatVdo::from_bytes(&mut br)?;

            let binding = product.to_le_bytes();
            let mut br = BitReader::new(Cursor::new(&binding));
            let product_vdo = Pd3p2ProductVdo::from_bytes(&mut br)?;

            Ok(Pd3p2DiscoverIdentityResponse {
                header: Default::default(),
                id_header_vdo,
                cert_stat,
                product_vdo,
                product_type_vdo,
            })
        }

        fn read_identity(
            &mut self,
            path: &str,
        ) -> Result<(u32, u32, u32, [Pd3p2ProductTypeVdo; 3])> {
            self.set_path(&format!("{}/{}", path, "cert_stat"))?;
            let cert_stat = self.read_u32()?;
            self.set_path(&format!("{}/{}", path, "id_header"))?;
            let id_header = self.read_u32()?;
            self.set_path(&format!("{}/{}", path, "product"))?;
            let product = self.read_u32()?;
            let mut product_type_vdo = [
                Pd3p2ProductTypeVdo::default(),
                Pd3p2ProductTypeVdo::default(),
                Pd3p2ProductTypeVdo::default(),
            ];
            for (i, vdo) in product_type_vdo.iter_mut().enumerate() {
                self.set_path(&format!("{}/product_type_vdo{}", path, i + 1))?;
                let value = self.read_u32()?;
                if value != 0 {
                    *vdo = Pd3p2ProductTypeVdo::n(value).ok_or(Error::ParseError {
                        field: "product_type_vdo".to_string(),
                        value,
                        #[cfg(feature = "backtrace")]
                        backtrace: std::backtrace::Backtrace::capture(),
                    })?;
                }
            }
            Ok((cert_stat, id_header, product, product_type_vdo))
        }
    }
}

/// A module to differentiate `SysfsWalker` from `MockSysfsWalker`. This is a
/// limitation of the `mockall` crate.
mod sysfs_walker {
    #[cfg(test)]
    use mockall::{automock, predicate::*};

    use std::ffi::OsStr;
    use std::path::Path;
    use std::path::PathBuf;

    use crate::Error;
    use crate::Result;

    #[cfg_attr(test, automock)]
    /// An abstraction for a directory entry. This abstracts away a directory
    /// entry so that it can be mocked.
    pub trait Entry {
        fn file_name(&self) -> &OsStr;
        fn path(&self) -> &Path;
    }

    impl Entry for walkdir::DirEntry {
        fn file_name(&self) -> &std::ffi::OsStr {
            self.file_name()
        }

        fn path(&self) -> &std::path::Path {
            self.path()
        }
    }

    /// A sysfs directory walker.
    pub struct SysfsWalker(Option<PathBuf>);

    #[cfg_attr(test, automock)]
    impl SysfsWalker {
        pub fn new() -> Result<Self> {
            Ok(Self(None))
        }

        pub fn set_path(&mut self, path: &str) -> crate::Result<()> {
            self.0 = Some(super::check_path(path)?);
            Ok(())
        }

        pub fn iter(&mut self) -> impl Iterator<Item = Result<Box<dyn Entry>>> {
            let path = self.0.take().expect("Path is not set");
            let wd = walkdir::WalkDir::new(path);
            WalkerIter(wd.into_iter())
        }
    }

    /// A mockable iterator for the directories.
    pub struct WalkerIter(pub walkdir::IntoIter);

    #[cfg_attr(test, automock)]
    impl Iterator for WalkerIter {
        type Item = Result<Box<dyn Entry>>;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|res| {
                // Convert from walkdir::Error into Error::DirError
                res.map_err(|walkdir_error| Error::DirError {
                    source: Box::new(walkdir_error),
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                })
                // Convert into a Box<dyn Entry>
                .map(|dir_entry| Box::new(dir_entry) as Box<dyn Entry>)
            })
        }
    }
}

pub struct SysfsBackend {
    /// Reads the sysfs files.
    reader: SysfsReader,
    /// Walks the sysfs directories.
    walker: SysfsWalker,
}

impl SysfsBackend {
    /// Initializes the sysfs backend.
    pub fn new() -> Result<Self> {
        let mut walker = SysfsWalker::new()?;
        walker.set_path(SYSFS_TYPEC_PATH)?;

        if walker.iter().count() == 1 {
            return Err(Error::NotSupported {
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            });
        }

        Ok(Self {
            reader: SysfsReader::new()?,
            walker: SysfsWalker::new()?,
        })
    }
}

impl OsBackend for SysfsBackend {
    fn capabilities(&mut self) -> Result<UcsiCapability> {
        let mut num_ports = 0;
        let mut num_alt_modes = 0;
        let mut pd_version = Default::default();
        let mut usb_type_c_version = Default::default();

        self.walker.set_path(SYSFS_TYPEC_PATH)?;
        for entry in self.walker.iter() {
            let entry = entry?;
            let entry_name = entry.file_name().to_string_lossy();

            let re = Regex::new(r"^port\d+$").unwrap();
            if re.is_match(&entry_name) {
                num_ports += 1;
                let path = &entry.path().to_string_lossy();
                self.walker.set_path(path)?;
                for port_entry in self.walker.iter() {
                    let port_entry = port_entry?;
                    let port_entry_name = port_entry.file_name().to_string_lossy();

                    let re = Regex::new(r"^port\d\.\d$").unwrap();
                    if re.is_match(&port_entry_name) {
                        num_alt_modes += 1;
                    }
                }

                let port_content_path =
                    format!("{}/usb_power_delivery_revision", entry.path().display());
                self.reader.set_path(&port_content_path)?;
                pd_version = self.reader.read_bcd()?;

                let port_content_path = format!("{}/usb_typec_revision", entry.path().display());
                self.reader.set_path(&port_content_path)?;
                usb_type_c_version = self.reader.read_bcd()?;
            }
        }

        let capabilities = UcsiCapability {
            num_connectors: num_ports,
            num_alt_modes,
            pd_version,
            usb_type_c_version,
            ..Default::default()
        };

        Ok(capabilities)
    }

    fn connector_capabilties(
        &mut self,
        connector_nr: usize,
    ) -> Result<crate::ucsi::UcsiConnectorCapability> {
        let path_str = format!("{SYSFS_TYPEC_PATH}/port{}", connector_nr);

        let port_content = format!("{}/{}", path_str, "power_role");
        self.reader.set_path(&port_content)?;

        let mut connector_capabilities = UcsiConnectorCapability {
            operation_mode: self.reader.read_opr()?,
            ..Default::default()
        };

        match connector_capabilities.operation_mode {
            ConnectorCapabilityOperationMode::Drp => {
                connector_capabilities.provider = true;
                connector_capabilities.consumer = true;
            }
            ConnectorCapabilityOperationMode::RdOnly => {
                connector_capabilities.consumer = true;
            }
            _ => {
                connector_capabilities.provider = true;
            }
        }

        if crate::is_chrome_os() {
            let port_content = format!(
                "{}/port{}-partner/{}",
                path_str, connector_nr, "usb_power_delivery_revision"
            );

            self.reader.set_path(&port_content)?;
            connector_capabilities.partner_pd_revision = self.reader.read_pd_revision()?;
        }

        Ok(connector_capabilities)
    }

    fn alternate_modes(
        &mut self,
        recipient: GetAlternateModesRecipient,
        connector_nr: usize,
    ) -> Result<Vec<UcsiAlternateMode>> {
        let mut alt_modes = vec![];

        loop {
            let num_alt_mode = alt_modes.len();
            let path_str = match recipient {
                crate::ucsi::GetAlternateModesRecipient::Connector => {
                    format!(
                        "{}/port{}/port{}.{}",
                        SYSFS_TYPEC_PATH, connector_nr, connector_nr, num_alt_mode
                    )
                }
                crate::ucsi::GetAlternateModesRecipient::Sop => {
                    format!(
                        "{}/port{}/port{}-partner/port{}-partner.{}",
                        SYSFS_TYPEC_PATH, connector_nr, connector_nr, connector_nr, num_alt_mode
                    )
                }
                crate::ucsi::GetAlternateModesRecipient::SopPrime => {
                    format!(
                        "{}/port{}-cable/port{}-plug0/port{}-plug0.{}",
                        SYSFS_TYPEC_PATH, connector_nr, connector_nr, connector_nr, num_alt_mode
                    )
                }
                _ => {
                    return Err(Error::NotSupported {
                        #[cfg(feature = "backtrace")]
                        backtrace: std::backtrace::Backtrace::capture(),
                    })
                }
            };

            let mut alt_mode = crate::ucsi::UcsiAlternateMode::default();

            let svid_path = format!("{}/{}", path_str, "svid");
            if self.reader.set_path(&svid_path).is_err() {
                break;
            }

            alt_mode.svid[0] = self.reader.read_hex_u32()?;

            let vdo_path = format!("{}/{}", path_str, "vdo");
            if self.reader.set_path(&vdo_path).is_err() {
                break;
            }

            alt_mode.vdo[0] = self.reader.read_hex_u32()?;
            alt_modes.push(alt_mode);
        }

        Ok(alt_modes)
    }

    fn cable_properties(&mut self, connector_nr: usize) -> Result<UcsiCableProperty> {
        let mut cable_property = UcsiCableProperty::default();
        let path_str = format!("{}/port{}-cable", SYSFS_TYPEC_PATH, connector_nr);

        let plug_type_path = format!("{}/{}", path_str, "plug_type");
        self.reader.set_path(&plug_type_path)?;
        cable_property.plug_end_type = self.reader.read_cable_plug_type()?;

        let cable_type_path = format!("{}/{}", path_str, "type");
        self.reader.set_path(&cable_type_path)?;
        cable_property.cable_type = self.reader.read_cable_type()?;

        let mode_support_path = format!(
            "{}/port{}-plug0/{}",
            SYSFS_TYPEC_PATH, connector_nr, "number_of_alternate_modes"
        );
        self.reader.set_path(&mode_support_path)?;
        cable_property.mode_support = self.reader.read_cable_mode_support()?;

        Ok(cable_property)
    }

    fn connector_status(&mut self, connector_nr: usize) -> Result<UcsiConnectorStatus> {
        let mut connector_status = UcsiConnectorStatus::default();

        let partner_path_str = format!(
            "{}/port{}/port{}-partner",
            SYSFS_TYPEC_PATH, connector_nr, connector_nr
        );
        connector_status.connect_status = Path::new(&partner_path_str).exists();

        let psy_path_str = format!(
            "{}/ucsi-source-psy-USBC000:00{}",
            SYSFS_PSY_PATH,
            connector_nr + 1
        );

        let online_path = format!("{}/{}", psy_path_str, "online");
        self.reader.set_path(&online_path)?;
        let ret = self.reader.read_hex_u32()?;

        if ret != 0 {
            let current_now_path = format!("{}/{}", psy_path_str, "current_now");
            self.reader.set_path(&current_now_path)?;
            let cur = self.reader.read_u32()? / 1000;

            let voltage_now_path = format!("{}/{}", psy_path_str, "voltage_now");
            self.reader.set_path(&voltage_now_path)?;
            let volt = self.reader.read_u32()? / 1000;

            let op_mw = (cur * volt) / (250 * 1000);

            let current_max_path = format!("{}/{}", psy_path_str, "current_max");
            self.reader.set_path(&current_max_path)?;
            let cur = self.reader.read_u32()? / 1000;

            let voltage_max_path = format!("{}/{}", psy_path_str, "voltage_max");
            self.reader.set_path(&voltage_max_path)?;
            let volt = self.reader.read_u32()? / 1000;

            let max_mw = (cur * volt) / (250 * 1000);

            connector_status.negotiated_power_level = (op_mw << 10) | (max_mw) & 0x3ff;
        }

        Ok(connector_status)
    }

    fn pd_message(
        &mut self,
        connector_nr: usize,
        recipient: PdMessageRecipient,
        response_type: PdMessageResponseType,
    ) -> Result<PdMessage> {
        match response_type {
            PdMessageResponseType::DiscoverIdentity => {
                Ok(PdMessage::Pd3p2DiscoverIdentityResponse(
                    self.reader.discover_identity(connector_nr, recipient)?,
                ))
            }
            _ => Err(Error::NotSupported {
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            }),
        }
    }

    fn pdos(
        &mut self,
        connector_nr: usize,
        partner_pdo: bool,
        _pdo_offset: u32,
        _nr_pdos: usize,
        pdo_type: PdoType,
        _source_capabilities_type: PdoSourceCapabilitiesType,
        _revision: BcdWrapper,
    ) -> Result<Vec<crate::pd::PdPdo>> {
        let mut pdos = Vec::new();

        let path_str = if partner_pdo {
            match pdo_type {
                PdoType::Source => {
                    format!(
                        "{}/port{}-partner/usb_power_delivery/source-capabilities",
                        SYSFS_TYPEC_PATH, connector_nr
                    )
                }
                PdoType::Sink => {
                    format!(
                        "{}/port{}-partner/usb_power_delivery/sink-capabilities",
                        SYSFS_TYPEC_PATH, connector_nr
                    )
                }
            }
        } else {
            match pdo_type {
                PdoType::Source => {
                    format!(
                        "{}/port{}/usb_power_delivery/source-capabilities",
                        SYSFS_TYPEC_PATH, connector_nr
                    )
                }
                PdoType::Sink => {
                    format!(
                        "{}/port{}/usb_power_delivery/sink-capabilities",
                        SYSFS_TYPEC_PATH, connector_nr
                    )
                }
            }
        };

        let port_path = format!("{SYSFS_TYPEC_PATH}/port{connector_nr}");
        self.walker.set_path(&port_path)?;
        for entry in self.walker.iter() {
            let entry = entry?;
            let entry_name = entry.file_name().to_string_lossy();
            let port_path = format!("{path_str}/{entry_name}");
            let port_path = Path::new(&port_path);

            let pdo = if entry_name.contains("fixed") {
                PdPdo::Pd3p2FixedSupplyPdo(self.reader.read_fixed_supply_pdo(port_path, pdo_type)?)
            } else if entry_name.contains("variable") {
                PdPdo::Pd3p2VariableSupplyPdo(
                    self.reader.read_variable_supply_pdo(port_path, pdo_type)?,
                )
            } else if entry_name.contains("battery") {
                PdPdo::Pd3p2BatterySupplyPdo(
                    self.reader.read_battery_supply_pdo(port_path, pdo_type)?,
                )
            } else if entry_name.contains("programmable") {
                PdPdo::Pd3p2AugmentedPdo(
                    self.reader
                        .read_programmable_supply_pdo(port_path, pdo_type)?,
                )
            } else {
                continue;
            };

            pdos.push(pdo);
        }

        Ok(pdos)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use mockall::{predicate::eq, Sequence};

    use crate::ucsi::{CablePropertyPlugEndType, CablePropertyType};

    use super::*;
    use sysfs_walker::MockEntry;
    #[double]
    use sysfs_walker::WalkerIter;

    #[test]
    fn test_get_capability() {
        let mut mock_reader = SysfsReader::default();
        let mut mock_walker = SysfsWalker::default();
        let path0 = format!("{SYSFS_TYPEC_PATH}/port0");
        let path1 = format!("{SYSFS_TYPEC_PATH}/port1");

        mock_walker
            .expect_set_path()
            .with(eq(SYSFS_TYPEC_PATH))
            .returning(|_| Ok(()));

        let mut mock_port0 = MockEntry::default();
        mock_port0
            .expect_file_name()
            .return_const(OsStr::new("port0").to_owned());
        mock_port0.expect_path().return_const(path0.clone().into());

        mock_walker
            .expect_set_path()
            .with(eq(path0.clone()))
            .return_once(|_| Ok(()));

        let mut mock_port00 = MockEntry::default();
        mock_port00
            .expect_file_name()
            .return_const(OsStr::new("port0.0").to_owned());
        let path00 = format!("{}/port0.0", path0);
        mock_port00.expect_path().return_const(path00.into());

        let mut mock_port01 = MockEntry::default();
        mock_port01
            .expect_file_name()
            .return_const(OsStr::new("port0.1").to_owned());
        let path11 = format!("{}/port0.1", path1);
        mock_port01.expect_path().return_const(path11.into());

        let mut mock_port02 = MockEntry::default();
        mock_port02
            .expect_file_name()
            .return_const(OsStr::new("port0.2").to_owned());
        let path12 = format!("{}/port0.2", path1);
        mock_port02.expect_path().return_const(path12.into());

        let mut port0_am_iter = WalkerIter::default();

        let mut seq = Sequence::new();
        port0_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port00))))
            .in_sequence(&mut seq);
        port0_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port01))))
            .in_sequence(&mut seq);
        port0_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port02))))
            .in_sequence(&mut seq);
        port0_am_iter
            .expect_next()
            .times(1)
            .return_once(|| None)
            .in_sequence(&mut seq);

        let path0_pd = format!("{}/usb_power_delivery_revision", path0);
        mock_reader
            .expect_set_path()
            .with(eq(path0_pd))
            .return_once(|_| Ok(()));

        let mut read_bcd_seq = Sequence::new();
        mock_reader
            .expect_read_bcd()
            .times(1)
            .returning(|| Ok(BcdWrapper(0x300)))
            .in_sequence(&mut read_bcd_seq);

        let path0_usbc_ver = format!("{}/usb_typec_revision", path0);
        mock_reader
            .expect_set_path()
            .with(eq(path0_usbc_ver))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_bcd()
            .times(1)
            .returning(|| Ok(BcdWrapper(0x300)))
            .in_sequence(&mut read_bcd_seq);

        let mut mock_port1 = MockEntry::default();
        mock_port1
            .expect_file_name()
            .return_const(OsStr::new("port1").to_owned());

        mock_port1.expect_path().return_const(path1.clone().into());

        let mut mock_port10 = MockEntry::default();
        mock_port10
            .expect_file_name()
            .return_const(OsStr::new("port1.0").to_owned());
        let path10 = format!("{}/port1.0", path1);
        mock_port10.expect_path().return_const(path10.into());

        let mut mock_port11 = MockEntry::default();
        mock_port11
            .expect_file_name()
            .return_const(OsStr::new("port1.1").to_owned());
        let path11 = format!("{}/port1.1", path1);
        mock_port11.expect_path().return_const(path11.into());

        let mut mock_port12 = MockEntry::default();
        mock_port12
            .expect_file_name()
            .return_const(OsStr::new("port1.2").to_owned());
        let path12 = format!("{}/port1.2", path1);
        mock_port12.expect_path().return_const(path12.into());

        let mut seq = Sequence::new();
        let mut port1_am_iter = WalkerIter::default();
        port1_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port10))))
            .in_sequence(&mut seq);
        port1_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port11))))
            .in_sequence(&mut seq);
        port1_am_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port12))))
            .in_sequence(&mut seq);
        port1_am_iter
            .expect_next()
            .times(1)
            .return_once(|| None)
            .in_sequence(&mut seq);

        let path1_pd = format!("{}/usb_power_delivery_revision", path1);
        mock_reader
            .expect_set_path()
            .with(eq(path1_pd))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_bcd()
            .times(1)
            .returning(|| Ok(BcdWrapper(0x300)))
            .in_sequence(&mut read_bcd_seq);

        let path1_usbc_ver = format!("{}/usb_typec_revision", path1);
        mock_reader
            .expect_set_path()
            .with(eq(path1_usbc_ver))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_bcd()
            .times(1)
            .returning(|| Ok(BcdWrapper(0x120)))
            .in_sequence(&mut read_bcd_seq);
        mock_walker
            .expect_set_path()
            .with(eq(path1))
            .return_once(|_| Ok(()));

        let mut seq = Sequence::new();
        let mut port_iter = WalkerIter::default();
        port_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port0))))
            .in_sequence(&mut seq);
        port_iter
            .expect_next()
            .times(1)
            .return_once(|| Some(Ok(Box::new(mock_port1))))
            .in_sequence(&mut seq);
        port_iter
            .expect_next()
            .times(1)
            .return_once(|| None)
            .in_sequence(&mut seq);

        let mut seq = Sequence::new();
        mock_walker
            .expect_iter()
            .times(1)
            .return_once(|| Box::new(port_iter))
            .in_sequence(&mut seq);
        mock_walker
            .expect_iter()
            .times(1)
            .return_once(|| Box::new(port0_am_iter))
            .in_sequence(&mut seq);
        mock_walker
            .expect_iter()
            .times(1)
            .return_once(|| Box::new(port1_am_iter))
            .in_sequence(&mut seq);

        let mut backend = SysfsBackend {
            reader: mock_reader,
            walker: mock_walker,
        };

        // Check that we can get the capabilities.
        let actual = backend.capabilities().unwrap();

        let expected = UcsiCapability {
            num_connectors: 2,
            num_alt_modes: 6,
            pd_version: BcdWrapper(0x300),
            usb_type_c_version: BcdWrapper(0x120),
            ..Default::default()
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_connector_capability() {
        let mut mock_reader = SysfsReader::default();
        let path0 = format!("{SYSFS_TYPEC_PATH}/port0");
        let path0_power_role = format!("{}/power_role", path0);

        mock_reader
            .expect_set_path()
            .with(eq(path0_power_role))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_opr()
            .returning(|| Ok(ConnectorCapabilityOperationMode::Drp));

        if crate::is_chrome_os() {
            let path0_pd = format!("{}/usb_power_delivery_revision", path0);
            mock_reader
                .expect_set_path()
                .with(eq(path0_pd))
                .return_once(|_| Ok(()));
            mock_reader
                .expect_read_pd_revision()
                .returning(|| Ok(0x300));
        }
        let mut backend = SysfsBackend {
            reader: mock_reader,
            walker: SysfsWalker::default(),
        };

        // Check that we can get the connector capabilities.
        let actual = backend.connector_capabilties(0).unwrap();
        let mut expected = UcsiConnectorCapability {
            operation_mode: ConnectorCapabilityOperationMode::Drp,
            provider: true,
            consumer: true,
            ..Default::default()
        };

        if crate::is_chrome_os() {
            expected.partner_pd_revision = 0x300;
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_alternate_modes() {
        let mut mock_reader = SysfsReader::default();
        let path0 = format!("{}/port0/port0.0", SYSFS_TYPEC_PATH);
        let path0_svid = format!("{}/svid", path0);
        let path0_vdo = format!("{}/vdo", path0);

        let mut sequence = Sequence::new();

        // Extracted from a test machine
        let svid0 = 32903;
        let vdo0 = 1;
        let svid1 = 65281;
        let vdo1 = 1842246;
        let svid2 = 16700;
        let vdo2 = 1;

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path0_svid))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .returning(move || Ok(svid0))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path0_vdo))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .return_once(move || Ok(vdo0))
            .times(1)
            .in_sequence(&mut sequence);

        let path1 = format!("{}/port0/port0.1", SYSFS_TYPEC_PATH);
        let path1_svid = format!("{}/svid", path1);
        let path1_vdo = format!("{}/vdo", path1);

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path1_svid))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .returning(move || Ok(svid1))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path1_vdo))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .return_once(move || Ok(vdo1))
            .times(1)
            .in_sequence(&mut sequence);

        let path2 = format!("{}/port0/port0.2", SYSFS_TYPEC_PATH);
        let path2_svid = format!("{}/svid", path2);
        let path2_vdo = format!("{}/vdo", path2);

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path2_svid))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .returning(move || Ok(svid2))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path2_vdo))
            .return_once(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        mock_reader
            .expect_read_hex_u32()
            .times(1)
            .return_once(move || Ok(vdo2))
            .times(1)
            .in_sequence(&mut sequence);

        let path3 = format!("{}/port0/port0.3", SYSFS_TYPEC_PATH);
        let path3_svid = format!("{}/svid", path3);
        mock_reader
            .expect_set_path()
            .times(1)
            .with(eq(path3_svid))
            .return_once(|_| {
                Err(Error::NotSupported {
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                })
            })
            .times(1)
            .in_sequence(&mut sequence);

        let mut backend = SysfsBackend {
            reader: mock_reader,
            walker: SysfsWalker::default(),
        };

        let alt_modes = backend
            .alternate_modes(crate::ucsi::GetAlternateModesRecipient::Connector, 0)
            .unwrap();

        assert_eq!(alt_modes[0].svid[0], svid0);
        assert_eq!(alt_modes[0].vdo[0], vdo0);
        assert_eq!(alt_modes[1].svid[0], svid1);
        assert_eq!(alt_modes[1].vdo[0], vdo1);
        assert_eq!(alt_modes[2].svid[0], svid2);
        assert_eq!(alt_modes[2].vdo[0], vdo2);
    }

    #[test]
    fn test_cable_properties() {
        let mut mock_reader = SysfsReader::default();
        let path_str = format!("{}/port{}-cable", SYSFS_TYPEC_PATH, 0);

        let plug_type_path = format!("{}/{}", path_str, "plug_type");
        mock_reader
            .expect_set_path()
            .with(eq(plug_type_path.clone()))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_cable_plug_type()
            .return_once(|| Ok(CablePropertyPlugEndType::UsbTypeC));

        let cable_type_path = format!("{}/{}", path_str, "type");
        mock_reader
            .expect_set_path()
            .with(eq(cable_type_path.clone()))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_cable_type()
            .return_once(|| Ok(CablePropertyType::Active));

        let mode_support_path = format!(
            "{}/port{}-plug0/{}",
            SYSFS_TYPEC_PATH, 0, "number_of_alternate_modes"
        );
        mock_reader
            .expect_set_path()
            .with(eq(mode_support_path.clone()))
            .return_once(|_| Ok(()));
        mock_reader
            .expect_read_cable_mode_support()
            .return_once(|| Ok(true));

        let mut backend = SysfsBackend {
            reader: mock_reader,
            walker: SysfsWalker::default(),
        };

        let actual = backend.cable_properties(0).unwrap();

        let expected = UcsiCableProperty {
            plug_end_type: CablePropertyPlugEndType::UsbTypeC,
            cable_type: CablePropertyType::Active,
            mode_support: true,
            ..Default::default()
        };

        assert_eq!(actual, expected);
    }
}
