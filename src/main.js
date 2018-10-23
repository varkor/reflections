"use strict";

performance.mark(PERFORMANCE_MARKERS.START_MARKER);

document.addEventListener("DOMContentLoaded", () => {
    const body = new Element(document.body);

    // For now, the input equations are stored in the location hash.
    let saved_settings;
    try {
        saved_settings = JSON.parse(decodeURIComponent(location.hash.slice(1)));
    } catch (err) {
        // Default settings.
        saved_settings = {
            mirror: ["t", "(t / 10) ^ 2"],
            figure: ["t", "x"],
            bindings: [["x", "0"]],
        };
    }
    let {
        mirror: mirror_equation,
        figure: figure_equation,
        bindings,
    } = saved_settings;
    bindings = new Map(bindings);

    const settings = new Map([
        ["threshold", "4"],
        ["method", "quadratic"],
        ["draw_normals", false],
        ["t_offset", "0"],
    ]);
    let reflection = null;

    const main = new Element("main");

    const [WIDTH, HEIGHT] = [640, 480];
    const canvas = new Graph(WIDTH, HEIGHT).append_to(main);
    let canvas_offset = canvas.element.getBoundingClientRect();
    let pointer = null;
    let drag_origin = null;
    let view = new View(canvas);

    canvas.listen("dblclick", () => {
        view = new View(canvas);
        rerender(false);
    });

    canvas.listen("mousedown", event => {
        if (event.button === 0) {
            pointer = new Pointer(event, canvas_offset);
            drag_origin = new Pointer(event, canvas_offset);
        }
    });

    canvas.listen("wheel", event => {
        event.preventDefault();
        const SCALE_DAMPING = 200;
        const scale_next = view.scale - event.deltaY / SCALE_DAMPING;
        if (pointer !== null) {
            const [offset_x, offset_y] = [
                pointer.x - canvas.width / 2,
                pointer.y - canvas.height / 2,
            ];
            const scale_delta = 2 ** -view.scale - 2 ** -scale_next;
            view.x += offset_x * scale_delta;
            view.y -= offset_y * scale_delta;
        }
        view.scale = scale_next;
        rerender(false);
    }, { passive: false });

    window.addEventListener("mousemove", event => {
        let request = false;
        if (pointer !== null && drag_origin !== null) {
            request = true;
        }
        const old_pointer = pointer !== null ? drag_origin : null;
        pointer = new Pointer(event, canvas_offset);
        drag_origin = old_pointer;
        if (request) {
            window.requestAnimationFrame(render);
        }
    });

    window.addEventListener("mouseup", event => {
        if (event.button === 0) {
            if (pointer !== null) {
                drag_origin = null;
            }
        }
    });

    window.addEventListener("mouseleave", () => pointer = null);

    performance.mark(PERFORMANCE_MARKERS.DOM_CONTENT_LOAD);
    RustWASMContext.connect().then(() => {
        canvas_offset = canvas.element.getBoundingClientRect();

        rerender(true);
    });

    let start = true;

    function render() {
        canvas.clear();

        function plot_normals(normals) {
            for (const entry of normals.entries()) {
                const [i, normal] = entry;
                canvas.context.fillStyle = canvas.context.strokeStyle = `hsl(${i}, 50%, 50%)`;
                canvas.plot_points(view, normal);
            }
        }

        if (reflection !== null) {
            reflection.points.then(data => {
                let [_, normals] = data;

                plot_normals(normals);
                reflection.plot(canvas, view, pointer);
            });
        }
    }

    function rerender(recompute) {
        // FIXME: make async to be faster
        if (recompute) {

            const t_offset = settings.get("t_offset");
            const bindings_new = new Map(bindings);
            bindings_new.set("t", `(t - ${t_offset})`);
            const present_vars = extract_variables();
            location.hash = encodeURIComponent(JSON.stringify({
                mirror: mirror_equation,
                figure: figure_equation,
                bindings: Array.from(bindings).filter(binding => present_vars.has(binding[0])),
            }));

            let [dx, dy] = [0, 0];
            if (pointer !== null && drag_origin !== null) {
                dx = pointer.x - drag_origin.x;
                dy = pointer.y - drag_origin.y;
                drag_origin.x = pointer.x;
                drag_origin.y = pointer.y;
            }
            const scale = 2 ** view.scale;
            view = { x: view.x - dx / scale, y: view.y + dy / scale, width: canvas.width, height: canvas.height, scale: view.scale };

            if (recompute) {
                recompute = false;

                if (!start) {
                    performance.mark(PERFORMANCE_MARKERS.START_MARKER);
                }
                reflection = new NonaffineReflection(
                    mirror_equation,
                    figure_equation,
                    bindings_new,
                    view,
                    settings,
                );
                reflection.points.then(data => {
                    let [_, normals] = data;

                    plot_normals(normals);
                    reflection.plot(canvas, view, pointer).then(() => {
                        if (start) {
                            PerformanceLogger.log(
                                PERFORMANCE_MARKERS.START_MARKER,
                                PERFORMANCE_MARKERS.DOM_CONTENT_LOAD,
                                PERFORMANCE_MARKERS.WASM_BINDGEN_CALL,
                                PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE,
                                PERFORMANCE_MARKERS.CANVAS_RENDER,
                            );
                            console.log("");
                        } else {
                            PerformanceLogger.log(
                                PERFORMANCE_MARKERS.START_MARKER,
                                PERFORMANCE_MARKERS.WASM_BINDGEN_CALL,
                                PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE,
                                PERFORMANCE_MARKERS.CANVAS_RENDER,
                            );
                            console.log("");
                        }
                        start = false;
                    });
                }).catch(() => {});
            }

            reflection = new NonaffineReflection(
                mirror_equation,
                figure_equation,
                bindings_new,
                view,
                settings,
            );
            reflection.points.then(() => window.requestAnimationFrame(render));
        }
        window.requestAnimationFrame(render)
    }

    const equation_container = new Div(["options"]).append_to(body);

    new Div(["dev-options"]).append_to(equation_container).append(
        // Rendering method.
        new Select(
            new Map(["Rasterisation", "Linear", "Quadratic"].map(m => [m.toLowerCase(), m])),
            settings.get("method"),
        ).listen("input", (_, self) => {
            settings.set("method", self.value);
            rerender(true);
        }),
        // Draw normals.
        new Div().append(
            new Label("Draw normals:").for_input(
                new Checkbox(settings.get("draw_normals"))
                    .for_which(self => self.id = "draw-normals")
                    .listen("input", (_, self) => {
                        settings.set("draw_normals", self.checked);
                        rerender(true);
                    }),
                false, // `follow` // FIXME
            ),
        ),
        // Approximation threshold.
        new Div()
            .append(new Label("Threshold:"))
            .append(new RangeSlider(0.0, settings.get("threshold"), 64.0).listen("input", (_, self) => {
                settings.set("threshold", self.value);
                rerender(true);
            })),
        // `t` offset.
        new Div()
            .append(new Label("t offset:"))
            .append(new RangeSlider(0.0, 0, 1.0, 0.01).listen("input", (_, self) => {
                settings.set("t_offset", self.value);
                rerender(true);
            })),
    );

    const figure = new ParametricEquationInput(() => {
        figure_equation = figure.components.map(({ value }) => value);
        extract_variables();
        rerender(true);
    }, ["figure"])
        .append_to(equation_container)
        .for_which(self => {
            self.components[0].id = "figure-equation";
            [self.components[0].value, self.components[1].value] = figure_equation;
        });
    new Label("Reflect the figure", figure.components[0]).precede(figure);

    const mirror = new ParametricEquationInput(() => {
        mirror_equation = mirror.components.map(({ value }) => value);
        extract_variables();
        rerender(true);
    }, ["mirror"])
        .append_to(equation_container)
        .for_which(self => {
            self.components[0].id = "mirror-equation";
            [self.components[0].value, self.components[1].value] = mirror_equation;
        });
    new Label("in the mirror", mirror.components[0]).precede(mirror);

    equation_container.add_text(", where:");

    const var_container = new Div(["variables"]).append_to(equation_container);

    const var_map = new Map();

    function extract_variables() {
        function add_var_slider(v) {
            const value = bindings.get(v);
            const value_text = new Div(["value"]).add_text(value);
            const container = new Div(["variable"])
                .append(new Element("span").add_text(`${v} = `).append(value_text))
                .append(new RangeSlider(-100, value, 100).listen("input", (_, self) => {
                    bindings.set(v, self.value);
                    value_text.element.childNodes[0].nodeValue = self.value;
                    rerender(true);
                }).for_which(self => var_map.set(v, self)));
            return container;
        }

        const vars = new Set(
            [...figure.components, ...mirror.components]
                .map(eq => [...new Equation(eq.value).variables()])
                .flat()
        );
        vars.delete("t"); // `t` is a special parameter.

        const all_vars = Array.from(new Set([...var_map.keys(), ...vars]));
        all_vars.sort();
        let prev_var = null;
        for (const v of all_vars) {
            if (vars.has(v)) {
                if (!var_map.has(v)) {
                    // We may already have a binding from the settings.
                    if (!bindings.has(v)) {
                        bindings.set(v, "0");
                    }
                    const var_slider = add_var_slider(v);
                    if (prev_var !== null) {
                        var_slider.follow(prev_var);
                    } else {
                        var_slider.append_to(var_container);
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

        return vars;
    }

    extract_variables();

    body.append(main);
});
