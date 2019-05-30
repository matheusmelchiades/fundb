const readline = require('readline')
const prompt = '🚀 FunDB '
const handleReadLine = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: `${prompt} 👉 `
});

module.exports = {
    path: __dirname + '/tables',
    samplePrompt: prompt,
    rl: handleReadLine //READ LINE
}