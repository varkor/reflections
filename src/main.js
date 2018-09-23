"use strict";

performance.mark("script-load");

class Canvas {
    constructor() {
        const canvas = this.element = document.createElement("canvas");
        this.context = canvas.getContext("2d");
        document.body.appendChild(canvas);

        [this.width, this.height] = [640, 480];
        const dpr = window.devicePixelRatio;
        [canvas.width, canvas.height] = [this.width * dpr, this.height * dpr];
        canvas.style.width = `${this.width}px`;
        canvas.style.height = `${this.height}px`;

        this.clear();

        this.context.transform(1, 0, 0, -1, 0, canvas.height);
    }

    clear() {
        this.context.fillStyle = "white";
        this.context.fillRect(0, 0, this.element.width, this.element.height);
    }

    plot_points(view, points, connect = false) {
        const dpr = window.devicePixelRatio;
        const width = 2;
        const vertices = new Path2D();
        const path = new Path2D();
        this.context.beginPath();
        for (const point of points) {
            const [px, py] = point;
            const [x, y] = [px + view.width / 2 - view.x, py + view.height / 2 - view.y];
            path.lineTo(x * dpr, y * dpr);
            vertices.moveTo(x * dpr, y * dpr);
            vertices.arc(x * dpr, y * dpr, width / 2 * dpr, 0, 2 * Math.PI);
        }
        if (connect) {
            this.context.save();
            this.context.lineWidth = width * dpr;
            this.context.stroke(path);
            this.context.restore();
        }
        this.context.fill(vertices);
    }

    plot_equation(view, points) {
        this.plot_points(view, points, true);
    }
}

class Rust {
    static connect() {
        let server = "http://0.0.0.0:8000/";
        let path = "target/wasm32-unknown-unknown/release/reflections_bg.wasm";
        performance.mark("dom-content-load");
        return window.wasm_bindgen(`${server}${path}`).then(() => {
            performance.mark("wasm-bindgen-load");
        });
    }

    static substitute(equation, bindings, t_offset) {
        for (const [variable, value] of bindings) {
            equation = equation.replace(new RegExp(`\\b${variable}\\b`, "g"), `(${value})`);
        }
        equation = equation.replace(/\bt\b/g, `(t-${t_offset})`);
        return equation;
    }

    static compute_reflection(fig_eqs, mirr_eqs, view, method, norms, thresh) {
        // console.log(canvas);
        const json = window.wasm_bindgen.proof_of_concept(view.x, view.y, fig_eqs[0], fig_eqs[1], mirr_eqs[0], mirr_eqs[1], method, norms, thresh);
        performance.mark("wasm-bindgen-call");
        try {
            const data = JSON.parse(json);
            performance.mark("wasm-bindgen-parse");
            return Promise.resolve(data);
        } catch (err) {
            console.error("Failed to parse Rust data:", json, err);
            return Promise.reject(new Error("failed to parse Rust data"));
        }
    }

    static plot_reflection(canvas, view, figure, mirror, reflection) {
        canvas.context.fillStyle = canvas.context.strokeStyle = "blue";
        canvas.plot_equation(view, figure);
        canvas.context.fillStyle = canvas.context.strokeStyle = "red";
        canvas.plot_equation(view, mirror);
        canvas.context.fillStyle = canvas.context.strokeStyle = "purple";
        canvas.plot_points(view, reflection);
        performance.mark("wasm-bindgen-plot");
    }

    static measure(...marks) {
        let start = marks.shift();
        while (marks.length > 0) {
            let end = marks.shift();
            performance.measure(end, start, end);
            start = end;
        }
        performance.getEntriesByType("measure").map(entry => {
            console.log(entry.name, entry.duration, `${Math.round(1000 / entry.duration * 10) / 10} fps`);
        });
        performance.clearMarks();
        performance.clearMeasures();
    }
}

document.addEventListener("DOMContentLoaded", () => {
    function create_input(action) {
        const input = document.createElement("input");
        input.type = "text";
        input.addEventListener("input", action);
        document.body.appendChild(input);
        return input;
    }

    function create_equation_input(action, class_name) {
        const x = create_input(action);
        const y = create_input(action);
        x.classList.add(class_name);
        y.classList.add(class_name);
        return [x, y];
    }

    // (x/1200)*t^2
    let [figure_equation_x, figure_equation_y, mirror_equation_x, mirror_equation_y, bindings] = location.hash !== "" ? decodeURIComponent(location.hash).slice(1).split(",") : ["t", "x", "t", "10", "x:0"];
    bindings = new Map(bindings.split(";").map(binding => binding.split(":")));
    let figure_equation = [figure_equation_x, figure_equation_y];
    let mirror_equation = [mirror_equation_x, mirror_equation_y];
    let thresh = 0;
    let method = "quads";
    let norms = false;
    let recompute = false;
    let t_offset = 0;

    let render = () => {};

    Rust.connect().then(() => {
        performance.mark("wasm-bindgen-load");
        let canvas = new Canvas();
        let view = { x: 0, y: 0, width: canvas.width, height: canvas.height };

        let pointer = {
            x: null,
            y: null,
            held: null,
        };

        let gview = view;

        canvas.element.addEventListener("mousedown", event => {
            if (event.button === 0) {
                pointer.x = event.pageX - window.scrollX;
                pointer.y = event.pageY - window.scrollY;
                pointer.held = {
                    x: pointer.x,
                    y: pointer.y,
                };
            }
        });

        window.addEventListener("mouseup", event => {
            if (event.button === 0) {
                pointer.x = event.pageX - window.scrollX;
                pointer.y = event.pageY - window.scrollY;
                if (pointer.held !== null) {
                    gview.x -= pointer.x - pointer.held.x;
                    gview.y += pointer.y - pointer.held.y;
                    pointer.held = null;
                }
            }
        });

        window.addEventListener("mousemove", event => {
            pointer.x = event.pageX - window.scrollX;
            pointer.y = event.pageY - window.scrollY;
            if (pointer.held !== null) {
                window.requestAnimationFrame(render);
            }
        });

        window.addEventListener("mouseleave", event => {
            pointer.x = null;
            pointer.y = null;
            pointer.held = null;
        });

        recompute = true;
        let old_data = null;
        let start = true;

        render = () => {
            // console.log("render");
            const fig_eq_x = Rust.substitute(figure_equation[0], bindings, t_offset);
            const fig_eq_y = Rust.substitute(figure_equation[1], bindings, t_offset);
            const mirr_eq_x = Rust.substitute(mirror_equation[0], bindings, t_offset);
            const mirr_eq_y = Rust.substitute(mirror_equation[1], bindings, t_offset);
            location.hash = encodeURIComponent(figure_equation[0] + "," + figure_equation[1] + "," + mirror_equation[0] + "," + mirror_equation[1] + "," + Array.from(bindings).map(binding => binding.join(":")).join(";"));

            let [dx, dy] = [0, 0];
            if (pointer.held !== null) {
                dx = pointer.x - pointer.held.x;
                dy = pointer.y - pointer.held.y;
            }
            view = { x: gview.x - dx, y: gview.y + dy, width: canvas.width, height: canvas.height };

            canvas.clear();

            function plot_normals(normals) {
                let i = 0;
                for (const normal of normals) {
                    canvas.context.fillStyle = canvas.context.strokeStyle = `hsl(${i}, 50%, 50%)`;
                    ++i;
                    canvas.plot_points(view, normal);
                }
            }

            if (recompute) {
                recompute = false;

                performance.mark("render-start");
                Rust.compute_reflection([fig_eq_x, fig_eq_y], [mirr_eq_x, mirr_eq_y], view, method, norms, thresh).then((data) => {
                    old_data = data;
                    let [mirror, normals, figure, reflection] = data;

                    plot_normals(normals);
                    Rust.plot_reflection(canvas, view, figure, mirror, reflection);

                    if (start) {
                        Rust.measure(
                            "script-load",
                            "dom-content-load",
                            "wasm-bindgen-load",
                            "wasm-bindgen-call",
                            "wasm-bindgen-parse",
                            "wasm-bindgen-plot",
                        );
                        console.log("");
                    } else {
                        Rust.measure(
                            "render-start",
                            "wasm-bindgen-call",
                            "wasm-bindgen-parse",
                            "wasm-bindgen-plot",
                        );
                        console.log("");
                    }
                    start = false;

                }).catch(() => {});
            } else {
                if (old_data !== null) {
                    const data = old_data;
                    let [mirror, normals, figure, reflection] = data;
                    plot_normals(normals);
                    Rust.plot_reflection(canvas, view, figure, mirror, reflection);
                }
            }
        }

        window.requestAnimationFrame(render);
    });

    const fig_text = document.createElement("span");
    fig_text.appendChild(document.createTextNode("Reflect the figure:"));
    document.body.appendChild(fig_text);

    const var_sliders = [];
    const var_container = document.createElement("div");

    function add_var_slider(v) {
        const container = document.createElement("div");
        const var_text = document.createElement("span");
        var_text.appendChild(document.createTextNode(`${v}:`));
        container.appendChild(var_text);
        const slider = document.createElement("input");
        slider.type = "range";
        slider.min = -100;
        slider.max = 100;
        if (!bindings.has(v)) {
            bindings.set(v, "0");
        }
        slider.value = bindings.get(v);
        slider.addEventListener("input", () => {
            bindings.set(v, slider.value);
            recompute = true;
            window.requestAnimationFrame(render);
        });
        var_sliders.push({
            var: v,
            element: slider,
        });
        container.appendChild(slider);
        var_container.appendChild(container);
    }

    function extract_variables() {
        const extract = eq => {
            return new Set(eq.match(/\b[a-z]\b/g));
        };
        const vars = new Set();
        [figure[0], figure[1], mirror[0], mirror[1]].map(eq => [...extract(eq.value)].forEach(x => vars.add(x)));
        vars.delete("t"); // special parameter
        for (const slider of var_sliders) {
            slider.element.parentElement.classList.toggle("hidden", !vars.has(slider.var));
            vars.delete(slider.var);
        }
        for (const new_var of vars) {
            add_var_slider(new_var);
        }
        return vars;
    }

    const figure = create_equation_input(() => {
        figure_equation = [figure[0].value, figure[1].value];
        extract_variables();
        recompute = true;
        window.requestAnimationFrame(render);
    }, "figure");
    [figure[0].value, figure[1].value] = figure_equation;

    const mirr_text = document.createElement("span");
    mirr_text.appendChild(document.createTextNode("in the mirror:"));
    document.body.appendChild(mirr_text);

    const mirror = create_equation_input(() => {
        mirror_equation = [mirror[0].value, mirror[1].value];
        extract_variables();
        recompute = true;
        window.requestAnimationFrame(render);
    }, "mirror");
    [mirror[0].value, mirror[1].value] = mirror_equation;

    document.body.appendChild(var_container);

    extract_variables();

    const thresh_slider = document.createElement("input");

    function method_button(name, force_thresh) {
        const button = document.createElement("button");
        button.appendChild(document.createTextNode(name));
        button.addEventListener("click", () => {
            method = name;
            thresh_slider.value = force_thresh ? "2" : "0";
            thresh = thresh_slider.value;
            recompute = true;
            window.requestAnimationFrame(render);
        });
        document.body.appendChild(button);
    }
    method_button("visual", true);
    method_button("kd", true);
    method_button("lines", true);
    method_button("quads", false);

    function append_text_span(element, text) {
        const span = document.createElement("span");
        span.appendChild(document.createTextNode(text));
        element.appendChild(span);
    }

    append_text_span(document.body, "draw normals:");

    const draw_normals = document.createElement("input");
    draw_normals.type = "checkbox";
    draw_normals.checked = norms;
    draw_normals.addEventListener("input", () => {
        norms = draw_normals.checked;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    document.body.appendChild(draw_normals);

    const thresh_container = document.createElement("div");
    thresh_slider.type = "range";
    thresh_slider.min = 0.0;
    thresh_slider.max = 64.0;
    thresh_slider.value = thresh;
    thresh_slider.addEventListener("input", () => {
        thresh = thresh_slider.value;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    append_text_span(thresh_container, "threshold:");
    thresh_container.appendChild(thresh_slider);
    document.body.appendChild(thresh_container);

    const t_offset_container = document.createElement("div");
    const t_offset_slider = document.createElement("input");
    t_offset_slider.type = "range";
    t_offset_slider.min = 0.0;
    t_offset_slider.max = 1.0;
    t_offset_slider.step = 0.01;
    t_offset_slider.value = 0;
    t_offset_slider.addEventListener("input", () => {
        t_offset = t_offset_slider.value;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    append_text_span(t_offset_container, "t offset:");
    t_offset_container.appendChild(t_offset_slider);
    document.body.appendChild(t_offset_container);
});
