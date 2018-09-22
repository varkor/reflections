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

    plot_lines(view, lines) {
        const dpr = window.devicePixelRatio;
        const width = 2;
        const vertices = new Path2D();
        const path = new Path2D();
        this.context.beginPath();
        for (const line of lines) {
            const [[px1, py1], [px2, py2]] = line;
            const [x1, y1] = [px1 + view.width / 2 - view.x, py1 + view.height / 2 - view.y];
            const [x2, y2] = [px2 + view.width / 2 - view.x, py2 + view.height / 2 - view.y];
            if (px1 !== px2 || py1 !== py2) {
                path.moveTo(x1 * dpr, y1 * dpr);
                path.lineTo(x2 * dpr, y2 * dpr);
                vertices.moveTo(x1 * dpr, y1 * dpr);
                vertices.arc(x1 * dpr, y1 * dpr, width / 2 * dpr, 0, 2 * Math.PI);
            }
            vertices.moveTo(x2 * dpr, y2 * dpr);
            vertices.arc(x2 * dpr, y2 * dpr, width / 2 * dpr, 0, 2 * Math.PI);
        }

        this.context.save();
        this.context.lineWidth = width * dpr;
        this.context.stroke(path);
        this.context.restore();
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

    static substitute(equation, bindings) {
        for (const [variable, value] of bindings) {
            equation = equation.replace(new RegExp(variable, "g"), `(${value})`);
        }
        return equation;
    }

    static compute_reflection(canvas, view, method, norms, thresh) {
        // console.log(canvas);
        const json = window.wasm_bindgen.proof_of_concept(view.x, view.y, canvas, method, norms, thresh);
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
        canvas.plot_lines(view, reflection);
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
            console.log(entry.name, entry.duration);
        });
        performance.clearMarks();
        performance.clearMeasures();
    }
}

document.addEventListener("DOMContentLoaded", () => {
    function create_equation_input(placeholder, action) {
        const input = document.createElement("input");
        input.type = "text";
        input.placeholder = placeholder;
        input.addEventListener("input", action);
        document.body.appendChild(input);
        return input;
    }

    // (x/1200)*t^2
    let [figure_equation, x_value] = location.hash !== "" ? location.hash.slice(1).split(",") : ["x", 0];
    let thresh = 2.0;
    let method = "quads";
    let norms = false;
    let recompute = false;

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
            const eq = Rust.substitute(figure_equation, new Map([["x", x_value]]));
            location.hash = figure_equation + "," + x_value;

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
                Rust.compute_reflection(eq, view, method, norms, thresh).then((data) => {
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

    // const mirror = create_equation_input("mirror");
    const figure = create_equation_input("figure", () => {
        figure_equation = figure.value;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    figure.value = figure_equation;

    function method_button(name) {
        const button = document.createElement("button");
        button.appendChild(document.createTextNode(name));
        button.addEventListener("click", () => {
            method = name;
            recompute = true;
            window.requestAnimationFrame(render);
        });
        document.body.appendChild(button);
    }
    method_button("visual");
    method_button("kd");
    method_button("lines");
    method_button("quads");

    const draw_normals = document.createElement("input");
    draw_normals.type = "checkbox";
    draw_normals.checked = norms;
    draw_normals.addEventListener("input", () => {
        norms = draw_normals.checked;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    document.body.appendChild(draw_normals);

    const offset_slider = document.createElement("input");
    offset_slider.type = "range";
    offset_slider.min = -100;
    offset_slider.max = 100;
    offset_slider.value = x_value;
    offset_slider.addEventListener("input", () => {
        x_value = offset_slider.value;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    document.body.appendChild(offset_slider);

    const thresh_slider = document.createElement("input");
    thresh_slider.type = "range";
    thresh_slider.min = 0.0;
    thresh_slider.max = 64.0;
    thresh_slider.value = thresh;
    thresh_slider.addEventListener("input", () => {
        thresh = thresh_slider.value;
        recompute = true;
        window.requestAnimationFrame(render);
    });
    document.body.appendChild(thresh_slider);
});
