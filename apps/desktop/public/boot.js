// Runs before anything else paints (a classic script: the CSP allows it, Chrome 67 runs it): the saved theme on <html>,
// so neither the loading screen nor the app flashes the other theme. Storage may be blocked: then the system's.
(function () {
  try {
    var theme = localStorage.getItem("oopz.theme");
    if (theme === "light" || theme === "dark") document.documentElement.setAttribute("data-theme", theme);
  } catch (e) {}
})();
