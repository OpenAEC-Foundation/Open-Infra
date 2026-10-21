import type { ComponentType } from 'react';
import { MapPin, Crosshair, Image as ImageIcon, Shapes, Globe, Server, Maximize } from 'lucide-react';
import { RibbonButton, RibbonSmallButton, RibbonGroup, useAppStore } from 'open-2d-studio';
import { openDialog, useDialogStack, type GisDialog } from './dialogState';
import { useSite } from '../gis/site';
import { LocationDialog } from './LocationDialog';
import { WmsDialog } from './WmsDialog';
import { WfsDialog } from './WfsDialog';
import { OsmDialog } from './OsmDialog';
import { ServerManagerDialog } from './ServerManagerDialog';
import { CoordinateDialog } from './CoordinateDialog';

/** Ribbon tab "GIS" — modelled on the FreeCAD GIS(2BIM) workbench toolbar. */
export function GisTabContent() {
  const site = useSite();
  return (
    <div className="ribbon-groups">
      <RibbonGroup label="Locatie">
        <RibbonButton icon={<MapPin size={24} />} label="Locatie" onClick={() => openDialog('location')}
          tooltip="Projectlocatie, coördinatenstelsel (RD, UTM, EPSG) en importgebied instellen" />
        <div className="ribbon-btn-stack">
          <RibbonSmallButton icon={<Crosshair size={14} />} label="Coördinaten" onClick={() => openDialog('coords')}
            tooltip="Coördinaten omrekenen tussen stelsels en punten plaatsen" />
          <RibbonSmallButton icon={<Maximize size={14} />} label="Zoom gebied" onClick={() => useAppStore.getState().zoomToFit()}
            tooltip="Zoom naar alles" />
        </div>
      </RibbonGroup>
      <RibbonGroup label="Kaartlagen">
        <RibbonButton icon={<ImageIcon size={24} />} label="WMS / Luchtfoto" onClick={() => openDialog('wms')}
          tooltip="Luchtfoto, kadastrale kaart, OSM-tegels of een andere WMS-laag als onderlegger" />
      </RibbonGroup>
      <RibbonGroup label="Vectordata">
        <RibbonButton icon={<Shapes size={24} />} label="WFS" onClick={() => openDialog('wfs')}
          tooltip="Objecten (percelen, panden, wegvakken, spoor…) uit een WFS-server importeren" />
        <RibbonButton icon={<Globe size={24} />} label="OpenStreetMap" onClick={() => openDialog('osm')}
          tooltip="Gebouwen, wegen, spoor, water en leidingen wereldwijd uit OpenStreetMap" />
      </RibbonGroup>
      <RibbonGroup label="Servers">
        <RibbonButton icon={<Server size={24} />} label="Serverbeheer" onClick={() => openDialog('servers')}
          tooltip="WMS-, WFS- en tegelservers beheren en eigen servers toevoegen" />
      </RibbonGroup>
      <RibbonGroup label="Project">
        <div style={{ fontSize: 11, lineHeight: 1.45, padding: '2px 6px', minWidth: 170, color: 'var(--theme-text-dim)' }}>
          {site ? (
            <>
              <div style={{ color: 'var(--theme-text)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{site.label}</div>
              <div>{site.crs} · {site.absolute ? 'absoluut' : 'relatief'}</div>
              <div>X {site.originE.toFixed(1)} · Y {site.originN.toFixed(1)}</div>
              <div>{site.width} × {site.height} m</div>
            </>
          ) : (
            <div>Geen locatie ingesteld</div>
          )}
        </div>
      </RibbonGroup>
    </div>
  );
}

const DIALOGS: Record<GisDialog, ComponentType> = {
  location: LocationDialog,
  wms: WmsDialog,
  wfs: WfsDialog,
  osm: OsmDialog,
  servers: ServerManagerDialog,
  coords: CoordinateDialog,
};

/** Rendered through the host's dialog registry, so dialogs survive ribbon tab switches. */
export function GisDialogHost() {
  const stack = useDialogStack();
  return (
    <>
      {stack.map((d, i) => {
        const C = DIALOGS[d];
        return <C key={`${d}-${i}`} />;
      })}
    </>
  );
}
