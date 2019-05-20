const crypto = require('crypto')
const Fs = require('fs');

const hashLength = 100;


function insert(obj) {
    const fd = Fs.openSync('./db/tools', 'a');
    console.log(fd)
    const data = JSON.stringify(obj)

    Fs.writeFileSync(fd, `${data}\n`);
    Fs.closeSync(fd)
}

function find(condition) {

    let data = [];

    const fd = Fs.openSync('./db/tools', 'r+');
    const lines = Fs.readFileSync(fd);

    console.log(JSON.parse(lines))

    Fs.closeSync(fd);

    return data;
}

// insert({ name: 'Vitor', age: 21 });
// insert({ name: 'Jose', age: 40 });
// insert({ name: 'Matheus', age: 21, o: 'teste' });

const data = find();