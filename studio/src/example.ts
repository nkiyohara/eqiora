export const EXAMPLE_SOURCE = `model controlled_decay {
  field state: 1 = 1;
  parameter rate: 1 / s = 0.8;
  relation decay continuous {
    derivative(state) + rate * state = 0;
  }
}
`;

export const SPATIAL_EXAMPLE_SOURCE = `model manufactured_poisson_plane {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;

  field potential on square as scalar_space: 1 = 0;
  parameter wave_number: 1 / m = 3.141592653589793;
  parameter source_scale: 1 / m ^ 2 = 19.739208802178716;

  relation balance continuous on square {
    -div(grad(potential))
      - source_scale
        * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) = 0; }
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
