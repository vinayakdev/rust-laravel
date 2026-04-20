<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

class CreateTeamMembershipsTable extends Migration
{
    public function up(): void
    {
        Schema::create('team_memberships', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->references('id')->on('users');
            $table->foreignId('role_id')->references('id')->on('roles');
            $table->foreignId('assigned_by')->nullable()->references('id')->on('users');
            $table->string('scope')->default('global');
            $table->timestamps();
            $table->unique(['user_id', 'role_id', 'scope']);
        });
    }
}
