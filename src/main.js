"use strict";

let log_index = PerformanceLogger.start();

// The whole project is a `<canvas>`-based experiment, so we can't really do anything interesting
// until the DOM has loaded.
document.addEventListener("DOMContentLoaded", () => {
    // For now, the input equations are stored in the location hash. At the very least, it makes
    // certain reflections easy to share.
    // Default settings.
    let saved_settings = {
        mirror: ["t", "(t / 10) ^ 2"],
        figure: ["t", "x"],
        sigma_tau: ["-s", "t"],
        bindings: [["x", { value: "0", min: "-256", max: "256", step: "1" }]],
        // Whether the view is locked (i.e. the user cannot pan or zoom). Particularly useful for
        // embedded canvases.
        locked: false,
    };
    try {
        Object.assign(saved_settings, JSON.parse(decodeURIComponent(location.hash.slice(1))));
    } catch {}
    let {
        mirror: mirror_equation,
        figure: figure_equation,
        sigma_tau: sigma_tau_equation,
        bindings,
        locked,
    } = saved_settings;
    let reflection = null;
    bindings = new Map(bindings.map(([name, data]) => {
        let { value, min, max, step } = data;
        [value, min, max, step] = [value, min, max, step].map(v => parseFloat(v));
        if (Number.isNaN(min) && Number.isNaN(max) && Number.isNaN(step) && Number.isNaN(value)) {
            // In this case, no values have been chosen, so we are free to make all choices
            // sensibly.
            [value, min, max, step] = [0, -256, 256, 1];
        } else {
            // Deal with partial invalid values. We make fairly arbitrary choices, under the
            // assumption this is an unlikely event anyway.
            if (Number.isNaN(min) && Number.isNaN(max)) {
                [min, max] = [-256, 256];
            } else if (Number.isNaN(min)) {
                min = max;
            } else if (Number.isNaN(max)) {
                max = min;
            }
            const mid = (min + max) / 2;
            if (min > max) {
                min = max = mid;
            }
            if (Number.isNaN(value)) {
                value = mid;
            }
            // Clamp to the valid range. We don't really care
            // whether the `value` is a multiple of `step`.
            value = Math.max(min, Math.min(max, value));
            if (Number.isNaN(step) || step <= 0) {
                step = (max - min) / 100;
                if (step > 1) {
                    // Avoid floating-point `step` where sensible.
                    step = Math.round(step);
                }
            }
            step = Math.min((max - min) / 2, step);
        }
        return [name, new Binding(value, min, max, step)];
    }));

    const settings = new Map([
        ["threshold", "4"],
        ["method", "quadratic"],
        ["t_offset", "0"],
        ["s_offset", "0"],
    ]);

    // There are two modes:
    // - Main:      the full Reflection Lab, in which the user has full control over the settings,
    //              such as equation input, variable sliders, etc.
    // - Embedded:  intended to be embedded into another webpage using an <iframe>. Here, the canvas
    //              is still interactive, and variables may be changed using sliders, but all other
    //              UI is hidden.
    const embedded = window.location.pathname.split("/").pop() === "embed.html";

    const body = new Element(document.body, embedded ? ["embedded"] : []);
    const main = new Element("main");

    const [WIDTH, HEIGHT] = [640, 480];
    // The canvas on which the equations are drawn.
    const canvas = new Graph(WIDTH, HEIGHT).append_to(main);
    // A hidden canvas to store the canvas before making temporary changes (e.g. hover effects).
    const buffer = new Graph(WIDTH, HEIGHT);
    // We need to make use of the bounding rect of the canvas, but it's not available until the
    // rendering frame after the canvas has been created.
    let canvas_offset = canvas.element.getBoundingClientRect();
    // The position of the cursor and where a mouse/touch drag started (if a drag is in progress).
    let [pointer, drag_origin] = [null, null];
    // The view represents the visible region on the canvas.
    let view = new View(canvas);

    // Double-clicking will reset the view.
    canvas.listen("dblclick", (event) => {
        if (event.button === 0) {
            if (!locked) {
                view = new View(canvas);
                render(false);
            }
        }
    });

    // Pressing initiates a drag, allowing the user to pan the view.
    canvas.listen("mousedown", (event) => {
        if (event.button === 0) {
            if (!locked) {
                pointer = new Pointer(event, canvas_offset);
                drag_origin = new Pointer(event, canvas_offset);
            }
        }
    });

    // Scrolling allows the user to zoom in or out.
    canvas.listen("wheel", (event) => {
        // We don't want to block scrolling if the canvas is embedded in a page (if it's not
        // embedded, the page won't scroll anyway, so there's no need to require Shift to zoom).
        if ((!embedded || event.shiftKey) && !locked) {
            event.preventDefault();
            // Scrolling is quite fast by default, so we want to slow it down.
            const SCALE_DAMPING = 200;
            const scale_next = view.scale - event.deltaY / SCALE_DAMPING;
            if (pointer !== null) {
                const [offset_x, offset_y] = [
                    pointer.x - canvas.width / 2,
                    pointer.y - canvas.height / 2,
                ];
                const scale_delta = 2 ** -view.scale - 2 ** -scale_next;
                view.origin[0] += offset_x * scale_delta;
                view.origin[1] -= offset_y * scale_delta;
            }
            view.scale = scale_next;
            render(false);
        }
    }, { passive: false });

    // If the cursor moves, and a drag is in progress, we need to update the position of the view
    // to pan it.
    window.addEventListener("mousemove", (event) => {
        if (canvas_offset !== null) {
            let request = false;
            if (pointer !== null && drag_origin !== null) {
                request = true;
            }
            const prev_pointer = pointer !== null ? drag_origin : null;
            pointer = new Pointer(event, canvas_offset);
            drag_origin = prev_pointer;
            if (request) {
                render(false);
            } else {
                // We're going to highlight the points closest to the pointer and their
                // corresponding pair (i.e. figure, mirror, image). The user can hover over any of
                // the three. However, in general, the user will be intending to hover over a
                // particular curve. It's unhelpful if we highlight any points close to the pointer,
                // as they could be unrelated. Therefore, we try to work out which of the three
                // curves the user is hovering over and then highlight any points on *that* curve
                // that are close to the pointer (even if there are points on the other curves that
                // are actually closer). The curve with the point that is closest to the pointer is
                // the one we choose.
                if (reflection !== null) {
                    // Clear the previous hover effect.
                    canvas.context.drawImage(buffer.element, 0, 0);

                    reflection.data.then((data) => {
                        // We generally want to allow hovering over multiple points that are close
                        // together, because the reflections might be in very different places.
                        // However, if we make the hover radius too large, it starts getting messy.
                        // `THRESHOLD` is the hover radius. However, if we don't find any in the
                        // hover radius at first, we allow it to expand a little (otherwise the user
                        // must be very precise in their hovering). Since no points were found in
                        // the initial radius, this expansion shouldn't cause too many extra points
                        // to be added.
                        const THRESHOLD = 8;
                        // How many `THRESHOLD`s we can expand before giving up (meaning the user
                        // isn't hovering near any curve).
                        const EXPAND = 1;

                        class ProximalPoints {
                            constructor() {
                                this.points = [];
                                this.distance = Infinity;
                            }

                            add_point(threshold, pointer, [x, y], value) {
                                const distance =
                                    Math.hypot(x - pointer.x, y - (canvas.height - pointer.y));
                                if (distance <= threshold) {
                                    this.points.push(value);
                                    this.distance = Math.min(this.distance, distance);
                                }
                            }
                        }

                        const reflection_prox = new ProximalPoints();
                        const figure_prox = new ProximalPoints();
                        const mirror_figure_prox = new ProximalPoints();
                        const mirror_reflection_prox = new ProximalPoints();
                        const prox = [
                            reflection_prox,
                            figure_prox,
                            mirror_figure_prox,
                            mirror_reflection_prox,
                        ];

                        const points = data.points.filter(triple => {
                            // The linear and rasterisation renderers currently do
                            // not support displaying the corresponding triple on
                            // hover. In these cases, they simply return NaN (which
                            // is converted into `null` in the JSON) for unknown
                            // points, so we skip such cases.
                            return !triple.flat().some(x => x === null);
                        })
                            .map((triple) => triple.map(p => Graph.adjust_point(view, p)))
                            .filter(x => x !== null);

                        for (
                            let threshold = THRESHOLD; threshold <= THRESHOLD * EXPAND; ++threshold
                        ) {
                            for (const point of points) {
                                const [r, f, m_f, m_r] = point;
                                reflection_prox.add_point(threshold, pointer, r, point);
                                figure_prox.add_point(threshold, pointer, f, point);
                                mirror_figure_prox.add_point(threshold, pointer, m_f, point);
                                mirror_reflection_prox.add_point(threshold, pointer, m_r, point);
                            }

                            const closest = prox.reduce((prev, cur) => {
                                return prev.distance <= cur.distance ? prev : cur;
                            });

                            if (closest.distance < Infinity) {
                                const dpr = window.devicePixelRatio;
                                const RADIUS = 6;

                                const lines = new Path2D();
                                const points = new Path2D();

                                for (let [r, f, m_f, m_r] of closest.points) {
                                    lines.moveTo(f[0] * dpr, f[1] * dpr);
                                    lines.lineTo(m_f[0] * dpr, m_f[1] * dpr);
                                    lines.moveTo(m_r[0] * dpr, m_r[1] * dpr);
                                    lines.lineTo(r[0] * dpr, r[1] * dpr);

                                    [r, f, m_f, m_r].forEach((p) => {
                                        points.moveTo(p[0] * dpr, p[1] * dpr);
                                        points.arc(
                                            p[0] * dpr,
                                            p[1] * dpr,
                                            RADIUS / 2 * dpr,
                                            0,
                                            2 * Math.PI,
                                        );
                                    });
                                }

                                canvas.context.strokeStyle = "hsla(0, 0%, 80%)";
                                canvas.context.stroke(lines);

                                canvas.context.fillStyle = "hsla(0, 0%, 50%)";
                                canvas.context.fill(points);

                                break;
                            }
                        }
                    });
                }
            }
        }
    });

    // Stop a drag event.
    window.addEventListener("mouseup", event => {
        if (event.button === 0) {
            if (!locked) {
                if (pointer !== null) {
                    drag_origin = null;
                }
            }
        }
    });

    // Clear the hover effect when the cursor leaves the canvas.
    canvas.listen("mouseleave", () => canvas.context.drawImage(buffer.element, 0, 0));

    window.addEventListener("mouseleave", () => pointer = null);

    PerformanceLogger.mark(log_index, PERFORMANCE_MARKERS.DOM_CONTENT_LOAD);
    // To start rendering reflections, we need to connect to the WASM context.
    RustWASMContext.connect(log_index).then(() => {
        canvas_offset = canvas.element.getBoundingClientRect();

        render(true, true);
    });

    /// Draw the mirror, figure and reflection. If `recompute`, then we recompute the equations.
    /// Otherwise, the existing equations are redrawn according to the new view.
    function render(recompute, start = false) {
        /// `draw_equations` assumes we have already computed the reflection and simply need to
        /// redraw it, on account of the view changing.
        function draw_equations(reflection, start, recomputed = false) {
            canvas.clear();

            if (reflection !== null) {
                reflection.plot(canvas, view, settings).then(() => {
                    // Save the reflection image in the buffer, so that we can draw on it without
                    // overwriting anything permanently.
                    buffer.context.drawImage(canvas.element, 0, 0);
                    if (start) {
                        // The first time we draw the reflection, we have some extra metrics to take
                        // into account when measuring the performance.
                        PerformanceLogger.log(
                            reflection.log_index,
                            PERFORMANCE_MARKERS.START_MARKER,
                            PERFORMANCE_MARKERS.DOM_CONTENT_LOAD,
                            PERFORMANCE_MARKERS.WASM_BINDGEN_CALL,
                            PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE,
                            PERFORMANCE_MARKERS.CANVAS_RENDER,
                        );
                        console.log("");
                    } else if (recomputed) {
                        PerformanceLogger.log(
                            reflection.log_index,
                            PERFORMANCE_MARKERS.START_MARKER,
                            PERFORMANCE_MARKERS.WASM_BINDGEN_CALL,
                            PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE,
                            PERFORMANCE_MARKERS.CANVAS_RENDER,
                        );
                        console.log("");
                    }
                });
            }
        }

        // Pan the view if the user has dragged on the canvas.
        let [dx, dy] = [0, 0];
        if (pointer !== null && drag_origin !== null) {
            dx = pointer.x - drag_origin.x;
            dy = pointer.y - drag_origin.y;
            drag_origin.x = pointer.x;
            drag_origin.y = pointer.y;
        }
        const scale = 2 ** view.scale;
        // Adjust the view to take account for zooming in/out.
        const view_adjusted = new View(canvas);
        view_adjusted.origin = [view.origin[0] - dx / scale, view.origin[1] + dy / scale];
        view_adjusted.scale = view.scale;
        view = view_adjusted;

        if (recompute) {
            // As a debugging aid, it's useful to offset the starting parameter for `t` and `s`, so
            // we allow them to be scaled in the range [0, 1).
            const [t_offset, s_offset] = [settings.get("t_offset"), settings.get("s_offset")];
            const bindings_new = new Map(bindings);
            // `t` and `s` are treated specially: their "current value" is actually
            // the offset we apply when we range over their domain.
            bindings_new.set("t", new Binding(parseFloat(t_offset), -256, 256, 1));
            bindings_new.set("s", new Binding(parseFloat(s_offset), -256, 256, 1));
            const present_vars = extract_variables();
            location.hash = encodeURIComponent(JSON.stringify({
                mirror: mirror_equation,
                figure: figure_equation,
                sigma_tau: sigma_tau_equation,
                bindings: Array.from(bindings)
                    .filter(binding => present_vars.has(binding[0]))
                    .map(binding => [binding[0], {
                        value: binding[1].value,
                        min: binding[1].min,
                        max: binding[1].max,
                        step: binding[1].step,
                    }]),
            }));
            // Save the HTML embed code in the `window` so that it's easily accessible.
            window.reflection = `
                <canvas class="embed"
                    data-figure-x="${figure_equation[0]}" data-figure-y="${figure_equation[1]}"
                    data-mirror-x="${mirror_equation[0]}" data-mirror-y="${mirror_equation[1]}"
                    data-sigma="${sigma_tau_equation[0]}" data-tau="${sigma_tau_equation[1]}"
                ></canvas>
            `.trim();

            if (!start) {
                log_index = PerformanceLogger.start();
            }

            const r = new NonaffineReflection(
                mirror_equation,
                figure_equation,
                sigma_tau_equation,
                bindings_new,
                view,
                settings,
                log_index,
            );
            r.data.then(() => {
                window.requestAnimationFrame(() => draw_equations(r, start, true));
            });
            // We store the latest reflection so that we have a reference to the
            // reflection data when we want to redraw it (when panning or zooming).
            reflection = r;
        } else {
            window.requestAnimationFrame(() => draw_equations(reflection, start));
        }
    }

    // Now we construct all the UI elements.
    const equation_container = new Div(["options"])
        .append_to(embedded ? new Div() : body);

    new Div(["dev-options"]).append_to(equation_container).append(
        // Rendering method.
        new Select(
            new Map(["Rasterisation", "Linear", "Quadratic"].map(m => [m.toLowerCase(), m])),
            settings.get("method"),
        ).listen("input", (_, self) => {
            settings.set("method", self.value);
            render(true);
        }),
        // Approximation threshold.
        new Div()
            .append(new Label("Threshold:"))
            .append(new RangeSlider(0.0, settings.get("threshold"), 64.0)
                .listen("input", (_, self) => {
                    settings.set("threshold", self.value);
                    render(true);
                })),
        // `t` offset.
        new Div()
            .append(new Label("t offset:"))
            .append(new RangeSlider(0.0, 0, 1.0, 0.01).listen("input", (_, self) => {
                settings.set("t_offset", self.value);
                render(true);
            })),
        // `s` offset.
        new Div()
            .append(new Label("s offset:"))
            .append(new RangeSlider(0.0, 0, 1.0, 0.01).listen("input", (_, self) => {
                settings.set("s_offset", self.value);
                render(true);
            })),
    );

    const figure = new ParametricEquationInput(() => {
        figure_equation = figure.components.map(({ value }) => value);
        extract_variables();
        render(true);
    }, ["x", "y"], ["figure"])
        .append_to(equation_container)
        .for_which(self => {
            self.components[0].id = "figure-equation";
            [self.components[0].value, self.components[1].value] = figure_equation;
        });
    new Label("Transform the figure", figure.components[0]).precede(figure);

    const mirror = new ParametricEquationInput(() => {
        mirror_equation = mirror.components.map(({ value }) => value);
        extract_variables();
        render(true);
    }, ["x", "y"], ["mirror"])
        .append_to(equation_container)
        .for_which(self => {
            self.components[0].id = "mirror-equation";
            [self.components[0].value, self.components[1].value] = mirror_equation;
        });
    new Label(" in the mirror", mirror.components[0]).precede(mirror);

    equation_container.add_text(
        ` where ${SPECIAL_VARIABLES.get("transformation")} = `
    );

    const sigma_tau = new ParametricEquationInput(() => {
        sigma_tau_equation = sigma_tau.components.map(({ value }) => value);
        extract_variables();
        render(true);
    }, [SPECIAL_VARIABLES.get("scaling"), SPECIAL_VARIABLES.get("translation")], ["sigma-tau"])
        .append_to(equation_container)
        .for_which(self => {
            self.components[0].id = "sigma-tau-equation";
            [self.components[0].value, self.components[1].value] = sigma_tau_equation;
        });

    new Button("Figure ⇄ Mirror").listen("click", () => {
        // Swap the figure and mirror equations.
        [
            [figure.components[0].value, figure.components[1].value],
            [mirror.components[0].value, mirror.components[1].value],
        ] = [
            [mirror.components[0].value, mirror.components[1].value],
            [figure.components[0].value, figure.components[1].value],
        ];
        for (const input of [figure, mirror].map(eq => eq.components).flat()) {
            input.element.dispatchEvent(new Event("input"));
        }
    }).append_to(equation_container);

    const var_container = new Div(["variables"]).append_to(new Div(
        embedded ? ["overlay"] : []
    ).append_to(
        embedded ? body : equation_container
    ));
    // We store the variable bindings, both for those variables that are currently free in an
    // equation and those that have previously been (so that variables have persistent values
    // even when being deleted temporarily).
    const var_map = new Map();

    /// Get the free variables from the equation inputs and produce sliders for the values of each
    /// one, as well as the special variables for scaling and translation.
    function extract_variables() {
        /// Add a new variable value slider.
        function add_var_slider(v) {
            const variable = bindings.get(v);
            const value_text = new Div(["value"]).add_text(variable.value);
            // Create the variable value slider itself.
            const slider = new RangeSlider(
                variable.min, variable.value, variable.max, variable.step,
            ).listen("input", (_, self) => {
                bindings.get(v).value = parseFloat(self.value);
                value_text.element.childNodes[0].nodeValue = self.value;
                render(true);
            }).for_which(self => var_map.set(v, self));
            // Variables with Greek letters, corresponding to special distinguished variables, are
            // highlighted.
            if (/[α-ω]/.test(v)) {
                slider.class_list.add("key");
            }
            const container = new Div(["variable"])
                .append(new Element("span")
                    .append(new Div(["name"]).add_text(`${v}`))
                    .add_text(" = ")
                    .append(value_text))
                .append(slider);
            return container;
        }

        // The free variables currently in the equation inputs.
        const vars = new Set(
            [...figure.components, ...mirror.components, ...sigma_tau.components]
                .map(eq => [...new Equation(eq.value).variables()])
                .flat()
        );
        vars.delete("t"); // `t` is a special parameter.
        vars.delete("s"); // `s` is a special parameter.

        // The combination of free variables and previously-set variables.
        const all_vars = Array.from(new Set([...var_map.keys(), ...vars]));
        all_vars.sort((a, b) => {
            if (/[α-ω]/.test(a) ^ /[α-ω]/.test(b)) {
                // Sort Greek letters before Latin ones.
                return b.localeCompare(a);
            }
            return a.localeCompare(b);
        });

        // Loop through all the variables. If a variable slider isn't present when it should be, we
        // add it in the correct (alphabetical) position. If a variable slider is present when it
        // shouldn't be, it's hidden.
        let prev_var = null;
        for (const v of all_vars) {
            if (vars.has(v)) {
                if (!var_map.has(v)) {
                    // We may already have a binding from the settings.
                    if (!bindings.has(v)) {
                        switch (v) {
                            case SPECIAL_VARIABLES.get("scaling"):
                                // `-1` is a good default setting for `scaling`, as we generally
                                // want to reflect.
                                bindings.set(v, new Binding(-1, -2, 2, -1));
                                break;
                            default:
                                bindings.set(v, new Binding(0, -256, 256, 1));
                                break;
                        }
                    }
                    const var_slider = add_var_slider(v);
                    if (prev_var !== null) {
                        var_slider.follow(prev_var);
                    } else {
                        var_slider.prepend_to(var_container);
                    }
                    var_map.set(v, var_slider);
                } else {
                    var_map.get(v).class_list.remove("hidden");
                }
            } else {
                var_map.get(v).class_list.add("hidden");
            }
            prev_var = var_map.get(v);
        }

        // If there are no variables, we hide the variable container (otherwise the padding takes
        // precedence over the width and we see part of the container).
        var_container.class_list.toggle("hidden", vars.size === 0);

        return vars;
    }

    // There might already be free variables depending on the loaded equations. (Additionally the
    // special variables will need sliders regardless.)
    extract_variables();

    // Add all the UI elements to the page.
    body.append(main);
});
