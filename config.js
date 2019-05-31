const readline = require('readline')
const prompt = '🚀 FunDB '
const handleReadLine = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: `${prompt} 👉 `
});

module.exports = {
    path: __dirname + '/tmp/tables',
    systemPath: __dirname + '/tmp/system',
    logPath: __dirname + '/tmp/logs',

    samplePrompt: prompt,
    rl: handleReadLine, //READ LINE,
}