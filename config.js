const readline = require('readline')
const prompt = '🚀 FunDB '
const fs = require('fs')
const handleReadLine = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: `${prompt} 👉 `
});

if (!fs.existsSync(__dirname + '/tmp')) {
    fs.mkdirSync(__dirname + '/tmp')
}

if (!fs.existsSync(__dirname + '/tmp/logs')) {
    fs.writeFileSync(__dirname + '/tmp/logs', '')
}

if (!fs.existsSync(__dirname + '/tmp/system')) {
    fs.writeFileSync(__dirname + '/tmp/system.json', '{}')
}


module.exports = {
    path: __dirname + '/tmp/tables',
    systemPath: __dirname + '/tmp/system.json',
    logPath: __dirname + '/tmp/logs',
    encoding: 'utf-8',

    samplePrompt: prompt,
    rl: handleReadLine, //READ LINE,
}



global.handleSystem = {
    stop: false,
    waitBy: '',
    message: () => {
        console.log('\n')
        console.log('#####################')
        console.log('       IN WAIT       ')
        console.log('#####################')
        console.log('\n')
    }
}