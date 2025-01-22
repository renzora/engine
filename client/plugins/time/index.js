window[id] = {
    id: id,
    hours: 22,
    minutes: 0,
    seconds: 0,
    days: 0,
    speedMultiplier: 100,
    active: true,
    daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],

    start: function() {

    },

    unmount: function() {

    },

    onRender: function() {
        this.update();
    },

    update: function() {
        if (!this.active) return;
        
        const gameSeconds = (game.deltaTime / 1000) * this.speedMultiplier;
        this.seconds += gameSeconds;

        if (this.seconds >= 60) {
            this.minutes += Math.floor(this.seconds / 60);
            this.seconds = this.seconds % 60;
        }
        if (this.minutes >= 60) {
            this.hours += Math.floor(this.minutes / 60);
            this.minutes = this.minutes % 60;
        }
        if (this.hours >= 24) {
            this.days += Math.floor(this.hours / 24);
            this.hours = this.hours % 24;
        }
    },

    display: function() {
        const pad = (num) => String(num).padStart(2, '0');
        const dayOfWeek = this.daysOfWeek[this.days % 7];
        return `${dayOfWeek} ${pad(this.hours)}:${pad(this.minutes)}`;
    }
}
