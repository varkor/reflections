"use strict";

performance.mark("script-load");

class Canvas {
    constructor(parent) {
        const canvas = this.element = document.createElement("canvas");
        this.context = canvas.getContext("2d");
        parent.appendChild(canvas);

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
        const scale = 2 ** view.scale;
        for (const [ux, uy] of points) {
            const [ox, oy] = [ux - view.x, uy - view.y];
            const [sx, sy] = [ox * scale, oy * scale];
            const [x, y] = [sx + view.width / 2, sy + view.height / 2];
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

    static plot_reflection(canvas, view, figure, mirror, reflection, pointer) {
        canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(190, 100%, 50%)";
        canvas.plot_equation(view, figure);
        canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(0, 100%, 50%)";
        canvas.plot_equation(view, mirror);
        // if (pointer.held === null && pointer.x !== null && pointer.y !== null) {
        //     const dpr = window.devicePixelRatio;
        //     const gradient = canvas.context.createRadialGradient(pointer.x * dpr, canvas.height * dpr - pointer.y * dpr, 0 * dpr, pointer.x * dpr, canvas.height * dpr - pointer.y * dpr, 32 * dpr);
        //     gradient.addColorStop(0, "yellow");
        //     gradient.addColorStop(1, "yellow");
        //     gradient.addColorStop(1, "purple");
        //     canvas.context.fillStyle = canvas.context.strokeStyle = gradient;
        // } else {
            canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(280, 100%, 50%)";
        // }
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
    function create_input(element, action) {
        const input = document.createElement("input");
        input.type = "text";
        input.addEventListener("input", action);
        element.appendChild(input);
        return input;
    }

    const equation_container = document.createElement("div");
    equation_container.classList.add("options");
    document.body.appendChild(equation_container);

    function create_equation_input(action, class_name) {
        const div = document.createElement("div");
        div.classList.add("parametric-equation");
        const x = create_input(div, action);
        const divider = document.createElement("span");
        divider.classList.add("divider");
        div.appendChild(divider);
        const y = create_input(div, action);
        x.classList.add(class_name);
        y.classList.add(class_name);
        equation_container.appendChild(div);
        return [x, y];
    }

    // (x/1200)*t^2
    let [figure_equation_x, figure_equation_y, mirror_equation_x, mirror_equation_y, bindings] = location.hash !== "" ? decodeURIComponent(location.hash).slice(1).split(",") : ["t", "x", "t", "10", "x:0"];
    bindings = new Map(bindings.split(";").map(binding => binding.split(":")));
    let figure_equation = [figure_equation_x, figure_equation_y];
    let mirror_equation = [mirror_equation_x, mirror_equation_y];
    let thresh = "4";
    let method = "quadratic";
    let norms = false;
    let recompute = false;
    let t_offset = "0";
    const lazy_rendering = true;

    let render = () => {};

    const main = document.createElement("main");

    Rust.connect().then(() => {
        performance.mark("wasm-bindgen-load");
        const canvas = new Canvas(main);
        const canvas_offset = canvas.element.getBoundingClientRect();
        let view = { x: 0, y: 0, width: canvas.width, height: canvas.height, scale: 0 };

        const pointer = {
            x: null,
            y: null,
            held: null,
        };

        let gview = view;

        canvas.element.addEventListener("dblclick", () => {
            gview = view = { x: 0, y: 0, width: canvas.width, height: canvas.height, scale: 0 };
            if (lazy_rendering) {
                window.requestAnimationFrame(render);
            }
        });

        canvas.element.addEventListener("mousedown", event => {
            if (event.button === 0) {
                pointer.x = event.pageX - window.scrollX - canvas_offset.left;
                pointer.y = event.pageY - window.scrollY - canvas_offset.top;
                pointer.held = {
                    x: pointer.x,
                    y: pointer.y,
                };
            }
        });

        canvas.element.addEventListener("wheel", event => {
            event.preventDefault();
            if (pointer.x !== null && pointer.y !== null) {
                const scale = 2 ** view.scale;
                const new_scale = 2 ** (view.scale - event.deltaY / 200);
                const [delta_x, delta_y] = [(pointer.x - canvas.width / 2) / scale, (pointer.y - canvas.height / 2) / scale];
                const [nu_x, nu_y] = [(pointer.x - canvas.width / 2) / new_scale, (pointer.y - canvas.height / 2) / new_scale];
                gview.x += delta_x - nu_x;
                gview.y -= delta_y - nu_y;
            }
            view.scale = view.scale - event.deltaY / 200;
            if (lazy_rendering) {
                window.requestAnimationFrame(render);
            }
        }, { passive: false });

        window.addEventListener("mouseup", event => {
            if (event.button === 0) {
                pointer.x = event.pageX - window.scrollX - canvas_offset.left;
                pointer.y = event.pageY - window.scrollY - canvas_offset.top;
                if (pointer.held !== null) {
                    const scale = 2 ** view.scale;
                    gview.x -= (pointer.x - pointer.held.x) / scale;
                    gview.y += (pointer.y - pointer.held.y) / scale;
                    pointer.held = null;
                    window.requestAnimationFrame(render);
                }
            }
        });

        window.addEventListener("mousemove", event => {
            pointer.x = event.pageX - window.scrollX - canvas_offset.left;
            pointer.y = event.pageY - window.scrollY - canvas_offset.top;
            if (pointer.held !== null || pointer.held === null && !lazy_rendering) {
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
            const present_vars = extract_variables();
            location.hash = encodeURIComponent(figure_equation[0] + "," + figure_equation[1] + "," + mirror_equation[0] + "," + mirror_equation[1] + "," + Array.from(bindings).filter(binding => present_vars.has(binding[0])).map(binding => binding.join(":")).join(";"));

            let [dx, dy] = [0, 0];
            if (pointer.held !== null) {
                dx = pointer.x - pointer.held.x;
                dy = pointer.y - pointer.held.y;
            }
            const scale = 2 ** view.scale;
            view = { x: gview.x - dx / scale, y: gview.y + dy / scale, width: canvas.width, height: canvas.height, scale: view.scale };

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
                    Rust.plot_reflection(canvas, view, figure, mirror, reflection, pointer);

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
                    Rust.plot_reflection(canvas, view, figure, mirror, reflection, pointer);
                }
            }

            if (!lazy_rendering) {
                window.requestAnimationFrame(render);
            }
        }

        window.requestAnimationFrame(render);
    });

    const fig_text = document.createElement("label");
    fig_text.appendChild(document.createTextNode("Reflect the figure"));
    equation_container.appendChild(fig_text);

    const var_sliders = [];
    const outer = document.createElement("div");

    const var_container = document.createElement("div");
    var_container.classList.add("variables");
    outer.appendChild(var_container);

    function add_var_slider(v) {
        const container = document.createElement("div");
        container.classList.add("variable")
        const var_text = document.createElement("span");
        container.appendChild(var_text);
        const slider = document.createElement("input");
        slider.type = "range";
        slider.min = -100;
        slider.max = 100;
        if (!bindings.has(v)) {
            bindings.set(v, "0");
        }
        slider.value = bindings.get(v);
        var_text.appendChild(document.createTextNode(`${v} = `));
        const value_text = document.createElement("div");
        value_text.classList.add("value");
        value_text.appendChild(document.createTextNode(slider.value));
        var_text.appendChild(value_text);
        slider.addEventListener("input", () => {
            bindings.set(v, slider.value);
            value_text.childNodes[0].nodeValue = slider.value;
            recompute = true;
            if (lazy_rendering) {
                window.requestAnimationFrame(render);
            }
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
        const old_vars = new Set(var_sliders.map(slider => slider.var));
        if (vars.size !== old_vars.size || !Array.from(vars).every(x => old_vars.has(x))) {
            var_sliders.length = 0;
            while (var_container.firstChild !== null) {
                var_container.removeChild(var_container.firstChild);
            }
            const var_add_order = Array.from(vars);
            var_add_order.sort();
            for (const new_var of var_add_order) {
                add_var_slider(new_var);
            }
        }
        return vars;
    }

    const figure = create_equation_input(() => {
        figure_equation = [figure[0].value, figure[1].value];
        extract_variables();
        recompute = true;
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    }, "figure");
    [figure[0].value, figure[1].value] = figure_equation;;
    fig_text.setAttribute("for", figure[0].id = "figure-equation");

    const mirr_text = document.createElement("label");
    mirr_text.appendChild(document.createTextNode("in the mirror"));
    equation_container.appendChild(mirr_text);

    const mirror = create_equation_input(() => {
        mirror_equation = [mirror[0].value, mirror[1].value];
        extract_variables();
        recompute = true;
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    }, "mirror");
    [mirror[0].value, mirror[1].value] = mirror_equation;
    mirr_text.setAttribute("for", mirror[0].id = "mirror-equation");

    equation_container.appendChild(document.createTextNode(", where:"));

    const dev_options = document.createElement("div");
    dev_options.classList.add("dev-options");
    const method_selector = document.createElement("select");
    for (const method of ["Rasterisation", "Linear", "Quadratic"]) {
        const option = document.createElement("option");
        option.value = method.toLowerCase();
        option.appendChild(document.createTextNode(method));
        method_selector.appendChild(option);
    }
    method_selector.value = method;
    method_selector.addEventListener("input", () => {
        method = method_selector.value;
        recompute = true;
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    });
    dev_options.appendChild(method_selector);
    equation_container.appendChild(dev_options);

    equation_container.appendChild(outer);

    extract_variables();

    const thresh_slider = document.createElement("input");

    function append_text_span(element, text) {
        const span = document.createElement("label");
        span.appendChild(document.createTextNode(text));
        element.appendChild(span);
        return span;
    }

    const normal_div = document.createElement("div");
    const draw_normal_label = append_text_span(normal_div, "Draw normals:");

    const draw_normals = document.createElement("input");
    draw_normals.type = "checkbox";
    draw_normals.checked = norms;
    draw_normals.addEventListener("input", () => {
        norms = draw_normals.checked;
        recompute = true;
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    });
    normal_div.appendChild(draw_normals);
    draw_normal_label.setAttribute("for", draw_normals.id = "draw-normals");
    dev_options.appendChild(normal_div);

    const thresh_container = document.createElement("div");
    thresh_slider.type = "range";
    thresh_slider.min = 0.0;
    thresh_slider.max = 64.0;
    thresh_slider.value = thresh;
    thresh_slider.addEventListener("input", () => {
        thresh = thresh_slider.value;
        recompute = true;
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    });
    append_text_span(thresh_container, "Threshold:");
    thresh_container.appendChild(thresh_slider);
    dev_options.appendChild(thresh_container);

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
        if (lazy_rendering) {
            window.requestAnimationFrame(render);
        }
    });
    append_text_span(t_offset_container, "t offset:");
    t_offset_container.appendChild(t_offset_slider);
    dev_options.appendChild(t_offset_container);

    document.body.appendChild(main);
});
