const levels = {
    DEBUG: 0,
    INFO: 1,
    WARN: 2,
    ERROR: 3
};

class Logger {
    constructor(name = 'app', level = levels.INFO) {
        this.name = name;
        this.level = level;
    }

    debug(message) {
        if (this.level <= levels.DEBUG) {
            console.log(`[DEBUG] [${this.name}] ${message}`);
        }
    }

    info(message) {
        if (this.level <= levels.INFO) {
            console.log(`[INFO] [${this.name}] ${message}`);
        }
    }

    warn(message) {
        if (this.level <= levels.WARN) {
            console.warn(`[WARN] [${this.name}] ${message}`);
        }
    }

    error(message) {
        if (this.level <= levels.ERROR) {
            console.error(`[ERROR] [${this.name}] ${message}`);
        }
    }
}

module.exports = Logger;
module.exports.levels = levels;
