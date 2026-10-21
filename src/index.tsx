/**
 * Open Infra — GIS workbench extension for Open 2D Studio / Open CAD Studio.
 *
 * Adds a "GIS" ribbon tab with project location and CRS, WMS raster layers,
 * WFS vector data, OpenStreetMap import and a server manager.
 */

import { useAppStore, dialogRegistry } from 'open-2d-studio';
import { GisTabContent, GisDialogHost } from './ui/GisTab';
import { getSite } from './gis/site';
import { ensureCrs } from './gis/crs';

const EXT_ID = 'open-infra';

const openInfraExtension = {
  onLoad() {
    useAppStore.getState().addExtensionRibbonTab({
      extensionId: EXT_ID, id: 'gis', label: 'GIS', order: 40, render: () => <GisTabContent />,
    });
    dialogRegistry.register({ id: 'open-infra-dialogs', Component: GisDialogHost });
    // A site saved with an epsg.io-defined CRS needs that definition again.
    const site = getSite();
    if (site) ensureCrs(site.crs).catch((err) => console.warn('[Open Infra] CRS laden mislukt:', err));
  },
  onUnload() {
    useAppStore.getState().removeExtensionRibbonTab(EXT_ID, 'gis');
    dialogRegistry.unregister('open-infra-dialogs');
  },
};

// ESM export for dev (direct import from Open 2D Studio)
export default openInfraExtension;

// CJS export for runtime extension loader
if (typeof module !== 'undefined') {
  module.exports = openInfraExtension;
}
