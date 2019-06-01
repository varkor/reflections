"use strict";

document.addEventListener("DOMContentLoaded", () => {
    const body = document.body;

    for (const canvas of body.querySelectorAll("canvas.embed")) {
        const settings = {
            mirror: [canvas.getAttribute("data-mirror-x"), canvas.getAttribute("data-mirror-y")],
            figure: [canvas.getAttribute("data-figure-x"), canvas.getAttribute("data-figure-y")],
            sigma_tau: [
                canvas.getAttribute("data-sigma") || "-s",
                canvas.getAttribute("data-tau") || "t",
            ],
            /* We use the default bindings settings. */
        };
        const iframe = new Iframe(`embed.html#${encodeURIComponent(JSON.stringify(settings))}`);
        canvas.insertAdjacentElement("afterend", iframe.element);
        canvas.remove();
    }
});
