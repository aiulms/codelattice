export const formatDate = (date) => {
    const d = new Date(date);
    return d.toISOString().split('T')[0];
};

export const capitalize = (str) => {
    if (!str) return '';
    return str.charAt(0).toUpperCase() + str.slice(1);
};

export const debounce = (fn, delay) => {
    let timeoutId;
    return (...args) => {
        clearTimeout(timeoutId);
        timeoutId = setTimeout(() => fn(...args), delay);
    };
};

const validators = {
    isEmail: (value) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value),
    isUrl: (value) => /^https?:\/\/.+/.test(value),
    isEmpty: (value) => !value || value.trim().length === 0
};

export default {
    formatDate,
    capitalize,
    debounce,
    validators
};
