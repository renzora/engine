const getTimestamp = () => {
  const now = new Date();
  return now.toLocaleTimeString('en-US', { 
    hour12: true,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });
};

export const logger = {
  log: (...args) => {
    console.log(`[${getTimestamp()}]`, ...args);
  },
  
  warn: (...args) => {
    console.warn(`[${getTimestamp()}]`, ...args);
  },
  
  error: (...args) => {
    console.error(`[${getTimestamp()}]`, ...args);
  },
  
  trace: (...args) => {
    console.log(`[${getTimestamp()}]`, ...args);
    console.trace();
  }
};

export const log = logger.log;
export const warn = logger.warn;
export const error = logger.error;
export const trace = logger.trace;