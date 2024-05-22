// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: © 2024 Google
// Ported from libtypec (Rajaram Regupathy <rajaram.regupathy@gmail.com>)

// Run with:
// cargo run --example c_example_lstypec --features c_api -- backend sysfs
// Or:
// cargo run --example c_example_lstypec --features c_api -- backend
// ucsi_debugfs
//
// This is an example of how to use the C API. It is similar in nature to
// the lstypec Rust binary.

#include "libtypec-rs.h"
#include <assert.h>
#include <complex.h>
#include <errno.h>
#include <stdio.h>

int c_example_lstypec(unsigned int backend) {
  int ret = 0;
  struct PdPdo *out_pdos = NULL;
  size_t out_npdos = 0;
  size_t out_mem_sz = 0;
  size_t connector_nr;
  unsigned int backend_type = backend ? backend : OsBackends_Sysfs;

  struct TypecRs *typec;
  ret = libtypec_rs_new(backend_type, &typec);
  if (typec == NULL) {
    fprintf(stderr, "Failed to create TypecRs instance\n");
    return ret;
  }

  // Get the capabilities
  struct UcsiCapability capabilities;
  ret = libtypec_rs_get_capabilities(typec, &capabilities);
  if (ret != 0) {
    fprintf(stderr, "Failed to get capabilities\n");
    return ret;
  }

  // Print the capabilities to the terminal
  UcsiCapability_printf(&capabilities);

  for (connector_nr = 0; connector_nr < capabilities.num_connectors;
       connector_nr++) {
    // Connector capabilities
    struct UcsiConnectorCapability connector;
    ret = libtypec_rs_get_conn_capabilities(typec, connector_nr, &connector);
    if (ret != 0) {
      fprintf(stderr, "Failed to get connector %zu\n", connector_nr);
      return ret;
    }

    // Print the connector to the terminal
    printf("Connector %zu Capability/Status\n", connector_nr);
    UcsiConnectorCapability_printf(&connector);

    // Connector PDOs (Source)
    ret = libtypec_rs_get_pdos(
        typec, connector_nr, false, 0, 0, PdoType_Source,
        PdoSourceCapabilitiesType_CurrentSupportedSourceCapabilities,
        capabilities.pd_version, &out_pdos, &out_npdos, &out_mem_sz);
    if (!ret) {
      assert(out_pdos);

      printf("Connector %zu Source PDOs\n", connector_nr);
      for (size_t i = 0; i < out_npdos; i++) {
        PdPdo_printf(&out_pdos[i]);
      }

      libtypec_rs_destroy_pdos(out_pdos, out_npdos, out_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get the Connector Source PDOs %zu\n",
              connector_nr);
      return ret;
    }
    printf("\n");

    // Connector PDOs (Sink)
    ret = libtypec_rs_get_pdos(
        typec, connector_nr, false, 0, 0, PdoType_Sink,
        PdoSourceCapabilitiesType_CurrentSupportedSourceCapabilities,
        capabilities.pd_version, &out_pdos, &out_npdos, &out_mem_sz);
    if (!ret) {
      assert(out_pdos);

      printf("Connector %zu Sink PDOs\n", connector_nr);

      for (size_t i = 0; i < out_npdos; i++) {
        PdPdo_printf(&out_pdos[i]);
      }

      libtypec_rs_destroy_pdos(out_pdos, out_npdos, out_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get the Connector Sink PDOs %zu\n",
              connector_nr);
      return ret;
    }
    printf("\n");

    // Cable properties
    struct UcsiCableProperty cable_props;
    ret = libtypec_rs_get_cable_properties(typec, connector_nr, &cable_props);
    if (!ret) {
      printf("Connector %zu Cable Properties\n", connector_nr);
      UcsiCableProperty_printf(&cable_props);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get cable properties\n");
      return ret;
    }
    printf("\n");

    // Supported alternate modes
    struct UcsiAlternateMode *alt_modes;
    size_t nmodes;
    size_t modes_mem_sz;
    ret = libtypec_rs_get_alternate_modes(
        typec, GetAlternateModesRecipient_Connector, connector_nr, &alt_modes,
        &nmodes, &modes_mem_sz);
    if (!ret) {
      printf("Connector %zu Alternate Modes\n", connector_nr);

      for (unsigned int i = 0; i < nmodes; i++) {
        UcsiAlternateMode_printf(&alt_modes[i]);
      }

      libtypec_rs_destroy_alternate_modes(alt_modes, nmodes, modes_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get connector alt modes\n");
      return ret;
    }
    printf("\n");

    // Cable
    ret = libtypec_rs_get_alternate_modes(
        typec, GetAlternateModesRecipient_SopPrime, connector_nr, &alt_modes,
        &nmodes, &modes_mem_sz);
    if (ret == 0) {
      printf("Connector %zu SOP' Alternate Modes\n", connector_nr);

      for (unsigned int i = 0; i < nmodes; i++) {
        UcsiAlternateMode_printf(&alt_modes[i]);
      }

      libtypec_rs_destroy_alternate_modes(alt_modes, nmodes, modes_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get SOP' alt modes\n");
      return ret;
    }
    printf("\n");

    struct PdMessage pd_msg;
    ret = libtypec_rs_get_pd_message(
        typec, connector_nr, PdMessageRecipient_SopPrime,
        PdMessageResponseType_DiscoverIdentity, &pd_msg);

    if (!ret) {
      printf("Connector %zu SOP' DiscoverIdentity PD Message\n", connector_nr);
      PdMessage_printf(&pd_msg);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr,
              "Failed to get the DiscoverIdentity PD message for SOP'\n");
      return ret;
    }
    printf("\n");

    // Partner
    ret = libtypec_rs_get_alternate_modes(typec, GetAlternateModesRecipient_Sop,
                                          connector_nr, &alt_modes, &nmodes,
                                          &modes_mem_sz);
    if (!ret) {
      printf("Connector %zu SOP Alternate Modes\n", connector_nr);
      for (unsigned int i = 0; i < nmodes; i++) {
        UcsiAlternateMode_printf(&alt_modes[i]);
      }
      libtypec_rs_destroy_alternate_modes(alt_modes, nmodes, modes_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get SOP alt modes\n");
      return ret;
    }
    printf("\n");

    ret = libtypec_rs_get_pd_message(
        typec, connector_nr, PdMessageRecipient_Sop,
        PdMessageResponseType_DiscoverIdentity, &pd_msg);

    if (!ret) {
      printf("Connector %zu SOP DiscoverIdentity PD Message\n", connector_nr);
      PdMessage_printf(&pd_msg);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr,
              "Failed to get the DiscoverIdentity PD message for SOP\n");
      return ret;
    }
    printf("\n");

    out_pdos = NULL;
    out_npdos = 0;
    out_mem_sz = 0;
    ret = libtypec_rs_get_pdos(
        typec, connector_nr, /*partner=*/true, 0, 0, PdoType_Source,
        PdoSourceCapabilitiesType_CurrentSupportedSourceCapabilities,
        capabilities.pd_version, &out_pdos, &out_npdos, &out_mem_sz);
    if (!ret) {
      assert(out_pdos);

      printf("Partner PDO data (Source)\n");
      for (size_t i = 0; i < out_npdos; i++) {
        PdPdo_printf(&out_pdos[i]);
      }
      libtypec_rs_destroy_pdos(out_pdos, out_npdos, out_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get Partner Source PDOs");
      return ret;
    }
    printf("\n");

    out_pdos = NULL;
    out_npdos = 0;
    out_mem_sz = 0;
    ret = libtypec_rs_get_pdos(
        typec, connector_nr, /*partner=*/true, 0, 0, PdoType_Sink,
        PdoSourceCapabilitiesType_CurrentSupportedSourceCapabilities,
        capabilities.pd_version, &out_pdos, &out_npdos, &out_mem_sz);
    if (!ret) {
      assert(out_pdos);

      printf("Partner PDO data (Sink)\n");
      for (size_t i = 0; i < out_npdos; i++) {
        PdPdo_printf(&out_pdos[i]);
      }

      libtypec_rs_destroy_pdos(out_pdos, out_npdos, out_mem_sz);
    } else if (ret != -ENOTSUP) {
      fprintf(stderr, "Failed to get Partner Sink PDOs");
      return ret;
    }

    printf("\n");
  }

  // Do not forget to destroy the library instance.
  libtypec_rs_destroy(typec);

  return 0;
}