// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

//! UCSI data structures and commands.

use bitstream_io::BitRead;
use bitstream_io::BitWrite;
use enumn::N;
use proc_macros::Printf;
use proc_macros::Snprintf;

use crate::BcdWrapper;
use crate::BitReader;
use crate::Error;
use crate::FromBytes;
use crate::Result;
use crate::ToBytes;

/// See UCSI - Table A-2 Parameter Values
pub const UCSI_MAX_NUM_ALT_MODE: usize = 128;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N, Copy)]
/// See Table 6-24: GET_ALTERNATE_MODES Command.
pub enum GetAlternateModesRecipient {
    Connector = 0,
    // SOP
    Sop = 1,
    // SOP'
    SopPrime = 2,
    // SOP''
    SopDoublePrime = 3,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N, Copy)]
pub enum PdoType {
    Sink,
    Source,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N, Copy)]
pub enum PdoSourceCapabilitiesType {
    CurrentSupportedSourceCapabilities,
    AdvertisedCapabilities,
    MaximumSupportedSourceCapabilities,
}

#[derive(Debug, Clone, Printf, Snprintf)]
pub enum UcsiCommand {
    /// This command is used to get the PPM capabilities.
    GetCapability,
    /// This command is used to get the capabilities of a connector.
    GetConnectorCapability {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    /// This command is used to get the Alternate Modes that the
    /// Connector/Cable/Attached Device is capable of supporting. If the
    /// Connector/Cable/Attached device does not support the number of Alternate
    /// Modes requested, starting from the value in the Alternate Mode offset
    /// field, it shall return only (six times the number of Alternate Mode)
    /// bytes to report the number of Alternate Modes it supports.
    GetAlternateModes {
        recipient: GetAlternateModesRecipient,
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    /// This command is used to get the list of Alternate Modes that are
    /// currently supported on the connector identified by this command. This
    /// shall be a subset of the complete list of Alternate Modes that the
    /// Connector is capable of supporting if the Alternate Mo de resources are
    /// being used by some other connector and are not available currently for
    /// this connector. The complete list of Alternate Modes that the Connector
    /// is capable of supporting is returned by GET_ALTERNATE_MODES with
    /// Connector as Recipient. For this command, the list is returned as a bit
    /// vector with one bit per Alternate Mode supported in the order that they
    /// were returned by the Connector in response to the GET_ALTERNATE_MODES
    /// commands.
    GetCamSupported {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    GetCurrentCam {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    /// This command is used to get the Sink or Source PDOs associated with the
    /// connector identified with the command. For the connector, this command
    /// can be used to get the Source PDOs/Capabilities
    GetPdos {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
        /// Should be set if the OPM wants to retrieve the PDOS of the device
        /// attached to the connector.
        partner_pdo: bool,
        /// Starting offset of the first PDO to be returned. Valid values are 0
        /// through 7 for the SPR range, 0 through 4 for the EPR range, 0
        /// through 11 for SPR and EPR ranges. Other values shall not be used.
        pdo_offset: u32,
        /// Number of PDOs to return starting from the PDO Offset. The number of
        /// PDOs to return is the value in this field plus 1.
        nr_pdos: usize,
        /// This field shall be set if the OPM wants to retrieve the Source PDOs
        /// otherwise it will retrieve the Sink PDOs.
        pdo_type: PdoType,
        /// The type of source capabilities requested, this field is only valid
        /// if `partner` is false and `pdo_type` is `PdoType::Sink`.
        source_capabilities_type: PdoSourceCapabilitiesType,
    },
    /// This command is used to get the Cable properties on the connector
    /// identified by this command.
    GetCableProperty {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    /// This command is used to get the current status of the connector
    /// identified by this command.
    GetConnectorStatus {
        /// This field shall be set to the connector being queried.
        connector_nr: usize,
    },
    /// This command is used to get the PD message from the connector
    GetPdMessage {
        /// This field shall be set to the connector being queried.
        connector_nr: u32,
        /// This field indicates the recipient of the PD message.
        recipient: PdMessageRecipient,
        /// Response message type.
        message_type: PdMessageResponseType,
    },
}

impl UcsiCommand {
    /// See UCSI 3.0 - Table A.1
    pub fn cmd_number(&self) -> u32 {
        match &self {
            UcsiCommand::GetCapability => 0x06,
            UcsiCommand::GetConnectorCapability { .. } => 0x07,
            UcsiCommand::GetAlternateModes { .. } => 0x0c,
            UcsiCommand::GetCamSupported { .. } => 0x0d,
            UcsiCommand::GetCurrentCam { .. } => 0xe,
            UcsiCommand::GetPdos { .. } => 0x10,
            UcsiCommand::GetCableProperty { .. } => 0x11,
            UcsiCommand::GetConnectorStatus { .. } => 0x12,
            UcsiCommand::GetPdMessage { .. } => 0x15,
        }
    }
}

impl ToBytes for UcsiCommand {
    fn to_bytes(&self, bw: &mut crate::BitWriter) -> Result<()> {
        let command = self.cmd_number();
        bw.write(8, command)?;
        match self {
            UcsiCommand::GetCapability => {}
            UcsiCommand::GetConnectorCapability { connector_nr } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
            }
            UcsiCommand::GetAlternateModes {
                recipient,
                connector_nr,
            } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(3, *recipient as u32)?;
                // Reserved
                bw.write(5, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
                // Reserved
                bw.write(1, 0)?;
            }
            UcsiCommand::GetCamSupported { connector_nr } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
            }
            UcsiCommand::GetCurrentCam { connector_nr } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
            }
            UcsiCommand::GetPdos {
                connector_nr,
                partner_pdo,
                pdo_offset,
                nr_pdos,
                pdo_type: src_or_sink_pdos,
                source_capabilities_type: pdo_type,
            } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
                bw.write(1, u32::from(*partner_pdo))?;
                bw.write(8, *pdo_offset)?;
                bw.write(2, *nr_pdos as u32)?;
                bw.write(1, *src_or_sink_pdos as u32)?;
                bw.write(2, *pdo_type as u32)?;
            }
            UcsiCommand::GetCableProperty { connector_nr } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
            }
            UcsiCommand::GetConnectorStatus { connector_nr } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr as u32 + 1)?;
            }
            UcsiCommand::GetPdMessage {
                connector_nr,
                recipient,
                message_type,
            } => {
                // Data length
                bw.write(8, 0)?;
                bw.write(7, *connector_nr + 1)?;
                bw.write(3, *recipient as u32)?;
                bw.write(16, 0)?;
                bw.write(6, *message_type as u32)?;
            }
        }

        bw.byte_align()?;
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf)]
pub enum PdMessage {
    /// Sink Capabilities Extended (Extended Message)
    Pd3p2SinkCapabilitiesExtended(crate::pd::Pd3p2SinkCapabilitiesExtended),
    /// Source Capabilities Extended (Extended Message)
    Pd3p2SourceCapabilitiesExtended(crate::pd::Pd3p2SourceCapabilitiesExtended),
    /// Battery Capabilities (Extended Message)
    Pd3p2BatteryCapabilities(crate::pd::Pd3p2BatteryCapData),
    /// Battery Status (Data Message)
    Pd3p2BatteryStatus(crate::pd::Pd3p2BatteryStatusData),
    /// Discover Identity Response – ACK, NAK or BUSY (Structured VDM)
    Pd3p2DiscoverIdentityResponse(crate::pd::Pd3p2DiscoverIdentityResponse),
    /// Revision (Data Message)
    Pd3p2Revision(crate::pd::Pd3p2RevisionMessageData),
}

/// This enum represents the recipient of the PD message.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N, Copy)]
pub enum PdMessageRecipient {
    /// The OPM wants to retrieve the USB PD response message from the
    /// identified connector.
    Connector,
    /// The OPM wants to retrieve the USB PD response message from the port
    /// partner of the identified connector.
    Sop,
    /// The OPM wants to retrieve the USB PD response message from the cable
    /// plug of the identified connector.
    SopPrime,
    /// The OPM wants to retrieve the USB PD response message from the cable
    /// plug of the identified connector.
    SopDoublePrime,
}

/// This enum represents the type of the PD response message.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Printf, Snprintf, N, Copy)]
pub enum PdMessageResponseType {
    /// Sink Capabilities Extended (Extended Message)
    SinkCapabilitiesExtended,
    /// Source Capabilities Extended (Extended Message)
    SourceCapabilitiesExtended,
    /// Battery Capabilities (Extended Message)
    BatteryCapabilities,
    /// Battery Status (Data Message)
    BatteryStatus,
    /// Discover Identity Response – ACK, NAK or BUSY (Structured VDM)
    DiscoverIdentity,
    /// Revision (Data Message)
    Revision,
    /// Reserved values.
    Reserved,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
/// This struct represents the GET_CONNECTOR_STATUS data.
pub struct UcsiConnectorStatus {
    /// A bitmap indicating the types of status changes that have occurred on
    /// the connector. See table 6-44 for a description of each bit.
    pub connector_status_change: ConnectorStatusChange,
    /// This field shall indicate the current power operation mode of the
    /// connector.
    pub power_operation_mode: PowerOperationMode,
    /// This field indicates the current connect status of the connector.
    pub connect_status: bool,
    /// This field shall indicate whether the connector is operating as a
    /// consumer or provider.
    pub power_direction: PowerDirection,
    /// This field is only valid when the Connect Status field is set.This field
    /// indicates the current mode the connector is operating in.
    pub connector_partner_flags: u32,
    /// This field indicates the type of connector partner detected on this
    /// connector.
    pub connector_partner_type: ConnectorPartnerType,
    /// This field shall return the currently negotiated power level.
    ///
    /// This field is only valid when the Connect Status field is set to one and
    /// the Power Operation Mode field is set to PD. Additionally, this is an
    /// optional field, and is valid only if the PPM has indicated support for
    /// the appropriate feature, as described in Section 6.5.6. See Tables 6-13,
    /// 6-14, 6-15 and 6-16 in the [USBPD] for additional information on the
    /// contents of this data structure
    pub negotiated_power_level: u32,
    /// This field is only valid if the connector is operating as a Sink.
    pub battery_charging_capability_status: BatteryChargingCapabilityStatus,
    /// A bitmap indicating the reasons why the Provider capabilities of the
    /// connector have been limited.
    ///
    /// See Table 6-45 for description of each bit.
    pub provider_capabilities_limited_reason: u32,
    /// This field indicates the USB Power Delivery Specification Revision
    /// Number the connector uses during an Explicit Contract.
    pub pd_version_operation_mode: u32,
    /// This field shall be set to 0 when the connection is in the direct
    /// orientation.
    pub orientation: ConnectorOrientation,
    /// This field shall indicate the status of the Sink Path.
    pub sink_path_status: SinkPathStatus,
    /// This field shall be set to one when the Reverse Current Protection
    /// happens.
    pub reverse_current_protection_status: bool,
    /// This field is set if the power reading is valid.
    pub power_reading_ready: bool,
    /// This field indicates the current resolution.
    pub scale_current: u32,
    /// This field is a peak current measurement reading.
    pub peak_current: u32,
    /// This field represents the moving average for the minimum time interval
    /// specified.
    pub average_current: u32,
    /// This field indicates the voltage resolution.
    pub scale_voltage: u32,
    /// This field is the most recent VBUS voltage measurement.
    pub voltage_reading: u32,
}

/// Connector Status Change Field Description for GET_CONNECTOR_STATUS. See
/// UCSI Table 6-44 for more information.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct ConnectorStatusChange {
    /// Bit 0: Reserved. Shall be set to zero.
    pub reserved1: bool,
    /// Bit 1: External Supply Change. When set to 1b, the OPM can get the
    /// current status of the supply attached to the PPM by using the
    /// GET_PDO command.
    pub external_supply_change: bool,
    /// Bit 2: Power Operation Mode Change. When set to 1b, the Power
    /// Operation Mode field in the STATUS Data Structure shall indicate the
    /// current power operational mode of the connector.
    pub power_operation_mode_change: bool,
    /// Bit 3: Attention. This bit shall be set to 1b when an LPM receives
    /// an attention from the port partner.
    pub attention: bool,
    /// Bit 4: Reserved. Shall be set to zero.
    pub reserved2: bool,
    /// Bit 5: Supported Provider Capabilities Change. When set to 1b, the
    /// OPM shall get the updated Power Data Objects by using the GET_PDOS
    /// command. The Supported Provider Capabilities Limited Reason field
    /// shall indicate the reason if the provider capabilities are limited.
    pub supported_provider_capabilities_change: bool,
    /// Bit 6: Negotiated Power Level Change. When set to 1b, the Request
    /// Data Object field in the STATUS Data Structure shall indicate the
    /// newly negotiated power level. Note that this bit shall be set by the
    /// PPM whenever a Power contract is established or renegotiated.
    pub negotiated_power_level_change: bool,
    /// Bit 7: PD Reset Complete. This bit shall be set to 1b when the PPM
    /// completes a PD Hard Reset requested by the connector partner.
    pub pd_reset_complete: bool,
    /// Bit 8: Supported CAM Change. When set to 1b, the OPM shall get the
    /// updated Alternate Modes supported by using the GET_CAM_SUPPORTED
    /// command.
    pub supported_cam_change: bool,
    /// Bit 9: Battery Charging Status Change. This bit shall be set to 1b
    /// when the Battery Charging status changes.
    pub battery_charging_status_change: bool,
    /// Bit 10: Reserved. Shall be set to zero.
    pub reserved3: bool,
    /// Bit 11: Connector Partner Changed. This bit shall be set to 1b when
    /// the Connector Partner Type field or Connector Partner Flags change.
    pub connector_partner_changed: bool,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Orientation.
pub enum ConnectorOrientation {
    /// The connection is in the normal orientation.
    #[default]
    Normal = 0,
    /// The connection is in the reverse orientation.
    Reverse = 1,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Sink Path Status.
pub enum SinkPathStatus {
    /// The Sink Path is not ready.
    #[default]
    NotReady = 0,
    /// The Sink Path is ready.
    Ready = 1,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Power Operation Mode.
pub enum PowerOperationMode {
    #[default]
    Reserved = 0,
    UsbDefaultOperation = 1,
    BatteryCharging = 2,
    PowerDelivery = 3,
    UsbTypeCCurrent1_5A = 4,
    UsbTypeCCurrent3A = 5,
    UsbTypeCCurrent5A = 6,
    Reserved2 = 7,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Power Direction.
pub enum PowerDirection {
    #[default]
    Consumer = 0,
    Provider = 1,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Connector Partner Type.
pub enum ConnectorPartnerType {
    #[default]
    Reserved = 0,
    DfpAttached = 1,
    UfpAttached = 2,
    PoweredCableNoUfpAttached = 3,
    PoweredCableUfpAttached = 4,
    DebugAccessoryAttached = 5,
    AudioAdapterAccessoryAttached = 6,
    Reserved2 = 7,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// This enum represents the Battery Charging Capability Status.
pub enum BatteryChargingCapabilityStatus {
    #[default]
    NotCharging = 0,
    NominalChargingRate = 1,
    SlowChargingRate = 2,
    VerySlowChargingRate = 3,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum CablePropertySpeedExponent {
    #[default]
    Bps = 0,
    Kbps = 1,
    Mbps = 2,
    Gbps = 3,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum CablePropertyPlugEndType {
    #[default]
    UsbTypeA,
    UsbTypeB,
    UsbTypeC,
    OtherNotUsb,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
pub enum CablePropertyType {
    #[default]
    Passive = 0,
    Active = 1,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
/// See UCSI Table 6-40: GET_CABLE_PROPERTY Data
pub struct UcsiCableProperty {
    /// Speed Exponent (SE). This field defines the base 10 exponent times 3,
    /// that shall be applied to the Speed Mantissa (SM) when calculating the
    /// maximum bit rate that this Cable supports.
    pub speed_exponent: CablePropertySpeedExponent,
    /// This field defines the mantissa that shall be applied to the SE when
    /// calculating the maximum bit rate.
    pub speed_mantissa: u32,
    /// Return the amount of current the cable is designed for in 50ma units.
    pub b_current_capability: u32,
    /// The PPM shall set this field to a one if the cable has a VBUS connection
    /// from end to end.
    pub vbus_in_cable: u32,
    /// The PPM shall set this field to one if the cable is an Active cable
    /// otherwise it shall set this field to zero if the cable is a Passive
    /// cable.
    pub cable_type: CablePropertyType,
    /// The PPM shall set this field to one if the lane directionality is
    /// configurable else it shall set this field to zero if the lane
    /// directionality is fixed in the cable.
    pub directionality: u32,
    pub plug_end_type: CablePropertyPlugEndType,
    /// This field shall only be valid if the CableType field is set to one.
    /// This field shall indicate that the cable supports Alternate Modes.
    pub mode_support: bool,
    /// Cable’s major USB PD Revision from the Specification Revision field of
    /// the USB PD Message Header
    pub cable_pd_revision: u32,
    /// See Table 6-41 in the [USBPD] for additional information on the contents
    /// of this field.
    pub latency: u32,
}

impl FromBytes for UcsiCableProperty {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let speed_exponent = reader.read::<u32>(2)?;
        let speed_exponent =
            CablePropertySpeedExponent::n(speed_exponent).ok_or_else(|| Error::ParseError {
                field: "speed_exponent".into(),
                value: speed_exponent,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        let speed_mantissa = reader.read::<u32>(14)?; // Read Speed Mantissa
        let b_current_capability = reader.read::<u32>(8)?; // Read Current Capability
        let vbus_in_cable = reader.read::<u32>(1)?; // Read VBUSInCable
        let cable_type = reader.read::<u32>(1)?; // Read CableType
        let cable_type = CablePropertyType::n(cable_type).ok_or_else(|| Error::ParseError {
            field: "cable_type".into(),
            value: cable_type,
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
        })?;
        let directionality = reader.read::<u32>(1)?; // Read Directionality
        let plug_end_type = reader.read::<u32>(2)?;
        let plug_end_type =
            CablePropertyPlugEndType::n(plug_end_type).ok_or_else(|| Error::ParseError {
                field: "plug_end_type".into(),
                value: plug_end_type,
                #[cfg(feature = "backtrace")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;
        let mode_support = reader.read_bit()?; // Read Mode Support
        let cable_pd_revision = reader.read::<u32>(2)?; // Read Cable PD Revision
        let latency = reader.read::<u32>(4)?; // Read Latency

        Ok(Self {
            speed_exponent,
            speed_mantissa,
            b_current_capability,
            vbus_in_cable,
            cable_type,
            directionality,
            plug_end_type,
            mode_support,
            cable_pd_revision,
            latency,
        })
    }
}

/// The response to a GET_ALTERNATE_MODES command.
///
/// See USCI 3.0 - Table 6.26.
#[repr(C)]
#[derive(Clone, PartialEq, Default, Printf, Snprintf)]
pub struct UcsiAlternateMode {
    pub svid: [u32; 2],
    pub vdo: [u32; 2],
}

impl FromBytes for UcsiAlternateMode {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let svid_0 = reader.read::<u32>(16)?; // Read SVID[0]
        let mid_0 = reader.read::<u32>(32)?; // Read MID[0]
        let svid_1 = reader.read::<u32>(16)?; // Read SVID[1]
        let mid_1 = reader.read::<u32>(32)?; // Read MID[1]

        Ok(Self {
            svid: [svid_0, svid_1],
            vdo: [mid_0, mid_1],
        })
    }
}

impl std::fmt::Debug for UcsiAlternateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vdo = format!("{:#08x}", self.vdo[0]);
        f.debug_struct("UcsiAlternateMode")
            .field("svid", &self.svid[0])
            .field("vdo", &vdo)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Printf, Snprintf)]
/// See UCSI - Table 6-29: GET_CAM_SUPPORTED Data
pub struct UcsiCamSupported {
    /// Whether an alternate mode is supported.
    pub cam_supported: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Printf, Snprintf)]
pub struct UcsiCurrentCam {
    /// Offsets into the list of Alternate Modes that the connector is
    /// currently operating in.
    ///
    /// This is an offset into the list of Alternate Modes supported by the PPM.
    /// If the connector is not operating in an alternate mode, the PPM shall
    /// set this field to 0xFF.
    pub current_alternate_mode: [usize; UCSI_MAX_NUM_ALT_MODE],
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// Connector capability data extended operation mode.
pub enum ConnectorCapabilityOperationMode {
    #[default]
    RpOnly,
    RdOnly,
    Drp,
    AnalogAudioAccessoryMode,
    DebugAccessoryMode,
    Usb2,
    Usb3,
    AlternateMode,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// Connector capability data extended operation mode.
pub enum ConnectorCapabilityExtendedOperationMode {
    #[default]
    Usb4Gen2,
    EprSource,
    EprSink,
    Usb4Gen3,
    Usb4Gen4,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf, N)]
/// Connector capability data miscellaneous capabilities.
pub enum ConnectorCapabilityMiscellaneousCapabilities {
    #[default]
    FwUpdate,
    Security,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
/// The response to a `GET_CONNECTOR_CAPABILITY` command.
/// See UCSI - Table 6-17: GET_CONNECTOR_CAPABILTY Data
pub struct UcsiConnectorCapability {
    /// This field shall indicate the mode that the connector can support.
    ///
    /// Note: Additional capabilities are described in the Extended Operation
    /// Mode field.
    pub operation_mode: ConnectorCapabilityOperationMode,
    /// True only when the operation mode is DRP or Rp only. This shall be true
    /// if the connector is capable of providing power on this connector.
    /// [Either PD, USB Type-C Current or BC 1.2].
    pub provider: bool,
    /// This field is valid only when the operation mode is DRP or Rd only. This
    /// shall be true if the connector is capable of consuming power on this
    /// connector. [Either PD, USB Type-C Current or BC 1.2].
    pub consumer: bool,
    /// This field is valid only when the operation mode is DRP or Rp only or Rd
    /// only. This shall be true if the connector is capable of accepting swap
    /// to DFP
    pub swap_to_dfp: bool,
    /// This field is valid only when the operation mode is DRP or Rp only or Rd
    /// only. This shall be true if the connector is capable of accepting swap
    /// to UFP
    pub swap_to_ufp: bool,
    /// This field is valid only when the operation mode is DRP. This field
    /// shall be true if the connector is capable of accepting swap to SRC.
    pub swap_to_src: bool,
    /// This bit is valid only when the operation mode is DRP. This bit shall be
    /// set to one if the connector is capable of accepting swap to SNK.
    pub swap_to_snk: bool,
    pub extended_operation_mode: ConnectorCapabilityExtendedOperationMode,
    pub miscellaneous_capabilities: ConnectorCapabilityMiscellaneousCapabilities,
    /// This is debug level information. True if the LPM supports this feature.
    /// Otherwise, false.
    pub reverse_current_protection_support: bool,
    /// Partner’s major USB PD Revision from the Specification Revision field of
    /// the USB PD message Header.
    pub partner_pd_revision: u8,
}

impl FromBytes for UcsiConnectorCapability {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let operation_mode_value = reader.read::<u32>(8)?;
        let operation_mode =
            ConnectorCapabilityOperationMode::n(operation_mode_value).ok_or_else(|| {
                Error::ParseError {
                    field: "operation_mode".into(),
                    value: operation_mode_value,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                }
            })?;
        let provider = reader.read_bit()?;
        let consumer = reader.read_bit()?;
        let swap_to_dfp = reader.read_bit()?;
        let swap_to_ufp = reader.read_bit()?;
        let swap_to_src = reader.read_bit()?;
        let swap_to_snk = reader.read_bit()?;
        let extended_operation_mode_value = reader.read::<u32>(8)?;
        let extended_operation_mode = ConnectorCapabilityExtendedOperationMode::n(
            extended_operation_mode_value,
        )
        .ok_or_else(|| Error::ParseError {
            field: "extended_operation_mode".into(),
            value: extended_operation_mode_value,
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
        })?;
        let miscellaneous_capabilities_value = reader.read::<u32>(4)?;
        let miscellaneous_capabilities =
            ConnectorCapabilityMiscellaneousCapabilities::n(miscellaneous_capabilities_value)
                .ok_or_else(|| Error::ParseError {
                    field: "miscellaneous_capabilities".into(),
                    value: miscellaneous_capabilities_value,
                    #[cfg(feature = "backtrace")]
                    backtrace: std::backtrace::Backtrace::capture(),
                })?;
        let reverse_current_protection_support = reader.read_bit()?;
        let partner_pd_revision = reader.read::<u8>(2)?;

        Ok(Self {
            operation_mode,
            provider,
            consumer,
            swap_to_dfp,
            swap_to_ufp,
            swap_to_src,
            swap_to_snk,
            extended_operation_mode,
            miscellaneous_capabilities,
            reverse_current_protection_support,
            partner_pd_revision,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct UcsiCapability {
    /// The supported PPM features.
    pub bm_attributes: UcsiBmAttributes,
    /// This field indicates the number of Connectors that this PPM supports.
    ///
    ///  A value of zero is illegal in this field.
    pub num_connectors: usize,
    /// Optional features supported.
    pub bm_optional_features: UcsiBmOptionalFeatures,
    /// This field indicates the number of Alternate Modes that this PPM
    /// supports.
    ///
    /// A value of zero in this field indicates that the PPM does not support
    /// Alternate Modes.
    ///
    /// The complete list of Alternate Modes supported by the PPM can be
    /// obtained using the GET_ALTERNATE_MODE command.
    ///
    /// The maximum number of Alternate Modes a PP can support is limited to
    /// MAX_NUM_ALT_MODE.
    pub num_alt_modes: usize,
    /// Battery Charging Specification Release Number.
    ///
    /// This field shall only be valid if the device indicates that it supports
    /// BC in the bmAttributes field.
    pub bc_version: BcdWrapper,
    /// USB Power Delivery Specification Revision Number.
    ///
    /// This field shall only be valid if the device indicates that it supports
    /// PD in the bmAttributes field.
    pub pd_version: BcdWrapper,
    /// USB Type-C Specification Release Number.
    ///
    /// This field shall only be valid if the device indicates that it supports
    /// USB Type -C in the bmAttributes field.
    pub usb_type_c_version: BcdWrapper,
}

impl FromBytes for UcsiCapability {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let bm_attributes = UcsiBmAttributes::from_bytes(reader)?;
        let num_connectors = reader.read::<u32>(7)? as usize;
        reader.skip(1)?; // Skip reserved bit
        let bm_optional_features = UcsiBmOptionalFeatures::from_bytes(reader)?;
        let num_alt_modes: usize = reader.read::<u32>(8)? as usize;
        reader.skip(8)?; // Skip reserved bits
        let bc_version = BcdWrapper(reader.read(16)?);
        let pd_version = BcdWrapper(reader.read(16)?);
        let usb_type_c_version = BcdWrapper(reader.read(16)?);

        Ok(Self {
            bm_attributes,
            bm_optional_features,
            num_connectors,
            num_alt_modes,
            bc_version,
            pd_version,
            usb_type_c_version,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct UcsiBmAttributes {
    /// Indicates whether this platform supports the Disabled State as defined
    /// in Section 4.5.2.2.1 in the [USBTYPEC].
    pub disabled_state_support: bool,
    /// Indicates whether this platform supports the Battery Charging
    /// Specification as per the value reported in the bcdBCVersion field.
    pub battery_charging: bool,
    /// Indicates whether this platform supports the USB Power Delivery
    /// Specification as per the value reported in the bcdPDVersion field.
    pub usb_power_delivery: bool,
    /// Indicates whether this platform supports power capabilities defined in
    /// the USB Type-C Specification as per the value reported in the
    /// bcdUSBTypeCVersion field.
    pub usb_type_c_current: bool,
    /// Indicates which sources are supported.
    pub bm_power_source: UcsiBmPowerSource,
}

impl FromBytes for UcsiBmAttributes {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let disabled_state_support: bool = reader.read_bit()?;
        let battery_charging: bool = reader.read_bit()?;
        let usb_power_delivery: bool = reader.read_bit()?;
        reader.skip(3)?; // Skip reserved bits
        let usb_type_c_current: bool = reader.read_bit()?;
        reader.skip(1)?; // Skip reserved bit
        let bm_power_source = UcsiBmPowerSource::from_bytes(reader)?;
        reader.skip(16)?; // Skip reserved bits

        Ok(Self {
            disabled_state_support,
            battery_charging,
            usb_power_delivery,
            usb_type_c_current,
            bm_power_source,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct UcsiBmOptionalFeatures {
    /// This feature indicates that the PPM supports the SET_CCOM command.
    pub set_ccom_supported: bool,
    /// This command is required and shall be set to always supported.
    pub set_power_level_supported: bool,
    /// This feature indicates that the PPM can report details about supported
    /// alternate modes to the OPM.
    pub alternate_mode_details_supported: bool,
    /// This feature indicates that the PPM allows the OPM to change the
    /// currently negotiated alternate mode using the SET_NEW_CAM command.
    pub alternate_mode_override_supported: bool,
    /// This feature indicates that the PPM can report details of Power Delivery
    /// Power Data Objects to the OPM.
    pub pdo_details_supported: bool,
    /// This feature indicates that the PPM supports the GET_CABLE_PROPERTY
    /// command.
    pub cable_details_supported: bool,
    /// This feature indicates that the PPM supports the External Supply Change
    /// notification.
    pub external_supply_notification_supported: bool,
    /// This feature indicates that the PPM supports the PD Reset notification.
    pub pd_reset_notification_supported: bool,
    /// This feature indicates that the LPM supports the GET_PD_MESSAGE command.
    pub get_pd_message_supported: bool,
    /// This feature indicates that the LPM supports GET_ATTENTION_VDO command.
    pub get_attention_vdo_supported: bool,
    /// This feature indicates that the PPM supports FW_UPDATE_REQUEST command.
    pub fw_update_request_supported: bool,
    /// This feature indicates that the PPM supports Power Level Notifications.
    pub negotiated_power_level_change_supported: bool,
    /// This feature indicates that the PPM supports SECURITY_REQUEST command.
    pub security_request_supported: bool,
    /// This feature indicates that the PPM supports SET_RETIMER_MODE command.
    pub set_retimer_mode_supported: bool,
    /// This feature indicates that the PPM supports the chunking of MESSAGE_IN
    /// and MESSAGE_OUT.
    pub chunking_supported: bool,
}

impl FromBytes for UcsiBmOptionalFeatures {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let set_ccom_supported: bool = reader.read_bit()?;
        let set_power_level_supported: bool = reader.read_bit()?;
        let alternate_mode_details_supported: bool = reader.read_bit()?;
        let alternate_mode_override_supported: bool = reader.read_bit()?;
        let pdo_details_supported: bool = reader.read_bit()?;
        let cable_details_supported: bool = reader.read_bit()?;
        let external_supply_notification_supported: bool = reader.read_bit()?;
        let pd_reset_notification_supported: bool = reader.read_bit()?;
        let get_pd_message_supported: bool = reader.read_bit()?;
        let get_attention_vdo_supported: bool = reader.read_bit()?;
        let fw_update_request_supported: bool = reader.read_bit()?;
        let negotiated_power_level_change_supported: bool = reader.read_bit()?;
        let security_request_supported: bool = reader.read_bit()?;
        let set_retimer_mode_supported: bool = reader.read_bit()?;
        let chunking_supported: bool = reader.read_bit()?;
        // This is not very clear, but this field is 24 bits and only 14 are
        // described in table 6-88
        reader.skip(9)?;

        Ok(Self {
            set_ccom_supported,
            set_power_level_supported,
            alternate_mode_details_supported,
            alternate_mode_override_supported,
            pdo_details_supported,
            cable_details_supported,
            external_supply_notification_supported,
            pd_reset_notification_supported,
            get_pd_message_supported,
            get_attention_vdo_supported,
            fw_update_request_supported,
            negotiated_power_level_change_supported,
            security_request_supported,
            set_retimer_mode_supported,
            chunking_supported,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default, Printf, Snprintf)]
pub struct UcsiBmPowerSource {
    pub ac_supply: bool,
    pub other: bool,
    pub uses_vbus: bool,
}

impl FromBytes for UcsiBmPowerSource {
    fn from_bytes(reader: &mut BitReader) -> Result<Self> {
        let ac_supply: bool = reader.read_bit()?;
        reader.skip(1)?; // Skip reserved bit
        let other: bool = reader.read_bit()?;
        reader.skip(3)?; // Skip reserved bits
        let uses_vbus: bool = reader.read_bit()?;
        reader.skip(1)?; // Skip reserved bit

        Ok(Self {
            ac_supply,
            other,
            uses_vbus,
        })
    }
}
