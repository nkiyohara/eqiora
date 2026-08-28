export const EXAMPLE_SOURCE = `model controlled_decay {
  field state: 1 = 1;
  parameter rate: 1 / s = 0.8;
  relation decay continuous {
    derivative(state) + rate * state = 0;
  }
}
`;

export const CAD_EXAMPLE_SOURCE = `model cad_semantic_selection {
  domain body = box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  domain z_lower = boundary(body, axis = 2, side = lower);
  domain z_upper = boundary(body, axis = 2, side = upper);
  representation geometry_space = continuum;
  field marker on body as geometry_space: 1 = 0;

  relation selected_boundary continuous on x_upper {
    trace(marker) = 0;
  }
}
`;

export const CAD_PREVIEW_MODEL_DIGEST = "cad14800".repeat(8);
