// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! The VDO data structures

#[cfg(feature = "backtrace")]
use std::backtrace::Backtrace;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

use bitstream_io::BitRead;
use enumn::N;
use proc_macros::Printf;
use proc_macros::Snprintf;

use crate::BcdWrapper;
use crate::BitReader;
use crate::Error;
use crate::FromBytes;
use crate::MilliOhm;
use crate::Result;

#[repr(C)]
/// Maximum VPD VBUS Voltage
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2MaxVbusVoltage {
    /// 20V
    V20 = 0,
    /// 30V (Deprecated)
    V30,
    /// 40V (Deprecated)
    V40,
    /// 50V (Deprecated)
    V50,
}

#[repr(C)]
/// Charge Through Support
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2ChargeThroughSupport {
    /// the VPD does not support Charge Through
    NotSupported = 0,
    /// the VPD supports Charge Through
    Supported,
}

#[repr(C)]
/// VPD VDO. USB PD 3.2 VPD VDO (Section 6.4.4.3.1.9)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pd3p2VpdVdo {
    /// HW Version 0000b…1111b assigned by the VID owner
    pub hw_version: u8,
    /// Firmware Version 0000b…1111b assigned by the VID owner
    pub firmware_version: u8,
    /// Version Number of the VDO (not this specification Version)
    pub vdo_version: u8,
    /// Maximum VPD VBUS Voltage
    pub max_vbus_voltage: Pd3p2MaxVbusVoltage,
    /// Charge Through Current Support
    pub charge_through_current_support: bool,
    /// VBUS Impedance
    pub vbus_impedance: MilliOhm,
    /// Ground Impedance
    pub ground_impedance: MilliOhm,
    /// Charge Through Support
    pub charge_through_support: Pd3p2ChargeThroughSupport,
}

impl FromBytes for Pd3p2VpdVdo {
    fn from_bytes(bit_reader: &mut BitReader) -> Result<Self> {
        let hw_version = bit_reader.read(4)?;
        let firmware_version = bit_reader.read(4)?;
        let vdo_version = bit_reader.read(3)?;
        let max_vbus_voltage = bit_reader.read(2)?;
        let max_vbus_voltage =
            Pd3p2MaxVbusVoltage::n(max_vbus_voltage).ok_or_else(|| Error::ParseError {
                field: "max_vbus_voltage".into(),
                value: max_vbus_voltage,
                #[cfg(feature = "backtrace")]
                backtrace: Backtrace::capture(),
            })?;
        let charge_through_current_support = bit_reader.read_bit()?;
        let vbus_impedance = bit_reader.read::<u32>(6)?.into();
        let ground_impedance = bit_reader.read::<u32>(6)?.into();
        let charge_through_support = bit_reader.read_bit()?;
        let charge_through_support = Pd3p2ChargeThroughSupport::n(charge_through_support)
            .ok_or_else(|| Error::ParseError {
                field: "charge_through_support".into(),
                value: u32::from(charge_through_support),
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        Ok(Self {
            hw_version,
            firmware_version,
            vdo_version,
            max_vbus_voltage,
            charge_through_current_support,
            vbus_impedance,
            ground_impedance,
            charge_through_support,
        })
    }
}

#[repr(C)]
/// UFP VDO Version
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2UfpVdoVersion {
    /// Version 1.3 = 011b
    V1_3 = 3,
}

#[repr(C)]
/// Device Capability
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2UfpVdoDeviceCapability {
    /// [USB 2.0] Device Capable
    Usb2_0 = 0,
    /// [USB 2.0] Device Capable (Billboard only)
    Usb2_0Billboard,
    /// [USB 3.2] Device Capable
    Usb3_2,
    /// [USB4] Device Capable
    Usb4,
}

#[repr(C)]
/// VCONN Power
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2UfpVdoVconnPower {
    /// 1W
    W1 = 0,
    /// 1.5W
    W1_5,
    /// 2W
    W2,
    /// 3W
    W3,
    /// 4W
    W4,
    /// 5W
    W5,
    /// 6W
    W6,
}

#[repr(C)]
/// Alternate Modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
pub enum Pd3p2UfpVdoAlternateModes {
    /// Supports [TBT3] Alternate Mode
    Tbt3 = 0,
    /// Supports Alternate Modes that reconfigure the signals on the [USB Type-C 2.3] connector – except for [TBT3].
    Reconfigurable,
    /// Supports Alternate Modes that do not reconfigure the signals on the [USB Type-C 2.3] connector
    NonReconfigurable,
}
#[repr(C)]
/// UFP VDO. See USB PD 3.2 - 6.4.4.3.1.4 UFP VDO
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pd3p2UfpVdo {
    /// Version Number of the VDO (not this specification Version)
    pub ufp_vdo_version: Pd3p2UfpVdoVersion,
    /// Device Capability
    pub device_capability: Pd3p2UfpVdoDeviceCapability,
    /// VCONN Power
    pub vconn_power: Pd3p2UfpVdoVconnPower,
    /// Indicates whether the AMA requires VCONN in order to function.
    pub vconn_required: bool,
    /// Indicates whether the AMA requires VBUS in order to function.
    pub vbus_required: bool,
    /// Alternate Modes
    pub alternate_modes: Pd3p2UfpVdoAlternateModes,
}

impl FromBytes for Pd3p2UfpVdo {
    fn from_bytes(bit_reader: &mut BitReader) -> Result<Self> {
        let ufp_vdo_version = bit_reader.read(3)?;
        let ufp_vdo_version =
            Pd3p2UfpVdoVersion::n(ufp_vdo_version).ok_or_else(|| Error::ParseError {
                field: "ufp_vdo_version".into(),
                value: ufp_vdo_version,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        bit_reader.skip(1)?; // Skip reserved bit
        let device_capability = bit_reader.read(4)?;
        let device_capability =
            Pd3p2UfpVdoDeviceCapability::n(device_capability).ok_or_else(|| Error::ParseError {
                field: "device_capability".into(),
                value: device_capability,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        bit_reader.skip(2)?; // Skip Connector Type (Legacy)
        bit_reader.skip(11)?; // Skip reserved bits
        let vconn_power = bit_reader.read(3)?;
        let vconn_power =
            Pd3p2UfpVdoVconnPower::n(vconn_power).ok_or_else(|| Error::ParseError {
                field: "vconn_power".into(),
                value: vconn_power,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        let vconn_required = bit_reader.read_bit()?;
        let vbus_required = bit_reader.read_bit()?;
        let alternate_modes = bit_reader.read(3)?;
        let alternate_modes =
            Pd3p2UfpVdoAlternateModes::n(alternate_modes).ok_or_else(|| Error::ParseError {
                field: "alternate_modes".into(),
                value: alternate_modes,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        Ok(Self {
            ufp_vdo_version,
            device_capability,
            vconn_power,
            vconn_required,
            vbus_required,
            alternate_modes,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, N, Printf, Snprintf)]
pub enum Pd3p2DfpVdoVersion {
    /// Version 1.2 = 010b
    Version12 = 0b010,
    // Values 011b…111b are Reserved and Shall Not be used
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, N, Printf, Snprintf)]
pub enum Pd3p2DfpVdoHostCapability {
    /// [USB 2] Host Capable
    Usb20 = 0,
    /// [USB 3] Host Capable
    Usb32 = 1,
    /// [USB 4] Host Capable
    Usb4 = 2,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Printf, Snprintf)]
/// See USB PD 3.2 - 6.4.4.3.1.5 DFP VDO
pub struct Pd3p2DfpVdo {
    /// Version Number of the VDO (not this specification Version)
    pub dfp_vdo_version: Pd3p2DfpVdoVersion,
    /// Host Capability Bit Description
    pub host_capability: Pd3p2DfpVdoHostCapability,
    /// Unique port number to identify a specific port on a multi-port device
    pub port_number: u32,
}

impl FromBytes for Pd3p2DfpVdo {
    fn from_bytes(bit_reader: &mut BitReader) -> Result<Self> {
        let dfp_vdo_version = bit_reader.read(3)?;
        let dfp_vdo_version =
            Pd3p2DfpVdoVersion::n(dfp_vdo_version).ok_or_else(|| Error::ParseError {
                field: "dfp_vdo_version".into(),
                value: dfp_vdo_version,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        bit_reader.skip(2)?;

        let host_capability = bit_reader.read(3)?;
        let host_capability =
            Pd3p2DfpVdoHostCapability::n(host_capability).ok_or_else(|| Error::ParseError {
                field: "host_capability".into(),
                value: host_capability,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        bit_reader.skip(2)?;

        let port_number = bit_reader.read(5)?;

        Ok(Pd3p2DfpVdo {
            dfp_vdo_version,
            host_capability,
            port_number,
        })
    }
}

/// The Discover Modes Command returns a list of zero to six VDOs, each of which
/// describes a Mode.
///
/// See 6.4.4.2.4 Object Position in USB-PD
pub const MAX_NUM_ALT_MODE: usize = 6;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2ProductVdo {
    /// Product ID (assigned by the manufacturer)
    product_id: u32,
    /// Device release number.
    device: BcdWrapper,
}

impl FromBytes for Pd3p2ProductVdo {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let product_id = reader.read(16)?;
        let device = reader.read(16)?;

        Ok(Pd3p2ProductVdo {
            product_id,
            device: BcdWrapper(device),
        })
    }
}

/// Contains the XID assigned by USB-IF to the product before certification in
/// binary format
///
/// See table 6.38 in the USB PD Specification for more information.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2CertStatVdo {
    /// The XID assigned by USB-IF to the product before certification in binary
    /// format.
    pub xid: u32,
}

impl FromBytes for Pd3p2CertStatVdo {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let xid = reader.read(32)?;

        Ok(Pd3p2CertStatVdo { xid })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N)]
/// See USBPDB 6.4.4.3.1.4
pub enum Pd3p2SopDfpProductType {
    NotADfp,
    PdUsbHub,
    PdUsbHost,
    PowerBrick,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N)]
pub enum Pd3p2SopUfpProductType {
    NotAUfp,
    PdUsbHub,
    PdUsbPeripheral,
    Psd,
    NotACablePlugOrVPD,
    PassiveCable,
    ActiveCable,
    VConnPoweredUsbDevice,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N)]
pub enum Pd3p2IdHeaderVdoConnectorType {
    TypecReceptacle = 2,
    TypecPlug = 3,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub struct Pd3p2IdHeaderVdo {
    /// Null-terminated vendor string.
    pub vendor: [u8; 32],
    /// USB Communications Capable as USB Host
    pub usb_host_capability: bool,
    /// USB Communications Capable as a USB Device
    pub usb_device_capability: bool,
    /// Indicates the type of Product when in UFP Data Role, whether a VDO will
    /// be returned and if so the type of VDO to be returned.
    pub sop_product_type_ufp: Pd3p2SopUfpProductType,
    /// Indicates whether or not the Product (either a Cable Plug or a device
    /// that can operate in the UFP role) is capable of supporting Modes.
    pub modal_operation_supported: bool,
    /// Indicates the type of Product when in DFP Data Role, whether a VDO will
    /// be returned and if so the type of VDO to be returned.
    pub sop_product_type_dfp: Pd3p2SopDfpProductType,
    /// A value identifying it as either a USB Type-C® receptacle or a USB
    /// Type-C® plug.
    pub connector_type: Pd3p2IdHeaderVdoConnectorType,
    /// Value of the Vendor ID assigned to them by USB-IF.
    pub usb_vendor_id: u32,
}

impl FromBytes for Pd3p2IdHeaderVdo {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let usb_host_capability = reader.read_bit()?;
        let usb_device_capability = reader.read_bit()?;

        let sop_product_type_ufp = reader.read(3)?;
        let sop_product_type_ufp =
            Pd3p2SopUfpProductType::n(sop_product_type_ufp).ok_or_else(|| Error::ParseError {
                field: "sop_product_type_ufp".into(),
                value: sop_product_type_ufp,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        let modal_operation_supported = reader.read_bit()?;
        let sop_product_type_dfp = reader.read(3)?;
        let sop_product_type_dfp =
            Pd3p2SopDfpProductType::n(sop_product_type_dfp).ok_or_else(|| Error::ParseError {
                field: "sop_product_type_dfp".into(),
                value: sop_product_type_dfp,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        let connector_type = reader.read(2)?;
        let connector_type =
            Pd3p2IdHeaderVdoConnectorType::n(connector_type).ok_or_else(|| Error::ParseError {
                field: "connector_type".into(),
                value: connector_type,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        reader.skip(5)?;

        let usb_vendor_id = reader.read(16)?;
        let hwdb = udev::Hwdb::new()?;
        let modalias = format!("usb:v{:04X}*", usb_vendor_id);

        let vendor_str = hwdb
            .query(modalias)
            .next()
            .map_or(std::ffi::OsString::from("Unknown"), |entry| {
                entry.name().to_os_string()
            });

        let vendor_str = CString::new(vendor_str.as_bytes())?;
        let mut vendor = [0u8; 32];
        let bytes = vendor_str.as_bytes_with_nul();
        vendor[..bytes.len()].copy_from_slice(bytes);

        Ok(Pd3p2IdHeaderVdo {
            vendor,
            usb_host_capability,
            usb_device_capability,
            sop_product_type_ufp,
            modal_operation_supported,
            sop_product_type_dfp,
            connector_type,
            usb_vendor_id,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum Pd3p2ProductTypeVdo {
    /// See USBPDB 6.4.4.3.1.6
    #[default]
    PassiveCableVdo,
    /// See USBPDB 6.4.4.3.1.7
    ActiveCableVdo,
    /// See USBPDB 6.4.4.3.1.9
    VpdVdo,
    /// See USBPDB 6.4.4.3.1.4
    UfpVdo,
    /// See USBPDB 6.4.4.3.1.5
    DfpVdo,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
/// A type representing the different types of VDO supported by the library.
pub enum Vdo {
    Pd3p2IdHeaderVdo(Pd3p2IdHeaderVdo),
    Pd3p2CertStatVdo(Pd3p2CertStatVdo),
    Pd3p2ProductTypeVdo(Pd3p2ProductTypeVdo),
    Pd3p2VpdVdo(Pd3p2VpdVdo),
    Pd3p2UfpVdo(Pd3p2UfpVdo),
    Pd3p2DfpVdo(Pd3p2DfpVdo),
}
