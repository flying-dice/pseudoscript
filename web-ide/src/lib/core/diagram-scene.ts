// Positioned data/feature scene DTOs — the shape `layout_data_scene` /
// `layout_feature_scene` (pseudoscript-emit) hand across the wasm boundary. The
// SvelteFlow wrapper and its overlay child both render from these, so the
// contract lives here, in one place, rather than being restated per component.

export type Rect = { x: number; y: number; w: number; h: number };

// --- data entity (ER) view --------------------------------------------------

export type EntityRow = { name: string; ty: string; target?: string | null };
export type DataEntity = {
  fqn: string;
  label: string;
  form: string;
  rows: EntityRow[];
  focal: boolean;
  rect: Rect;
};
export type DataLink = { from: string; to: string; field: string };
export type DataLayout = {
  of?: string | null;
  entities: DataEntity[];
  links: DataLink[];
  width: number;
  height: number;
};

// --- feature flow view ------------------------------------------------------

export type FeatureStep = { keyword: string; text: string; rect: Rect };
export type FeatureLayout = {
  entry?: string | null;
  name: string;
  target_label: string;
  steps: FeatureStep[];
  width: number;
  height: number;
};
